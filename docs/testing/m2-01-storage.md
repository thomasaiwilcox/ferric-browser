# M2-01 implementation evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: STORE-001 through STORE-005 (initial storage boundary).
- Files: `crates/ferric-browser-storage/Cargo.toml`,
  `crates/ferric-browser-storage/src/lib.rs`, `roots.rs`, `profiles.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, its QML surface, the CLI bootstrap,
  workspace `Cargo.toml`/`Cargo.lock`, `crates/ferric-browser-storage/src/downloads.rs`,
  and `docs/requirements.csv`.
- Observable result: `ProfileStore::open` creates a normal profile-local
  SQLite database, enables foreign keys/WAL/FULL synchronous mode, applies a
  five-second busy timeout, and installs the initial metadata tables in one
  clean-break schema initialization. Rust-owned APIs record deduplicated page metadata and
  visits, bookmarks, quickmarks, and explicitly eligible command history.
  Private mode is rejected before opening a database, so no private metadata
  can reach this durable owner. Root resolution supports XDG, an explicit
  absolute base, and a uniquely named temporary root with 0700 directories.
  The profile registry persists stable UUID-backed normal profile records by
  atomic replacement and rejects unsafe names or private entries. The Qt
  bridge acquires a private kernel-backed exclusive runtime lock per normal profile;
  multiple windows in one process share it, another owner is reported, and a process
  crash releases ownership without deleting the persistent informational lock file.
  It opens the selected normal profile's store under that UUID and
  records successful committed navigations and applies the configured history
  retention cutoff; command history is capped transactionally at 1,000 rows.
  History URL admission rejects credentials, fragments, the reviewed secret
  query-key set (including OAuth access/refresh tokens and client secrets),
  percent-encoded spellings of those keys, and malformed percent-encoded query
  names rather than persisting ambiguous input.
  Older schemas are refused without opening them for write; private profiles
  never open the store. `--basedir` and `--temp-basedir` are passed through to
  the same root selection and temporary roots are cleaned after the GUI exits.
- Evidence: `cargo test -p ferric-browser-storage --locked` (55 tests),
  `cargo test -p ferric-browser-engine-qt --locked`, `cargo test -p ferric-browser
  --locked`, and `cargo clippy -p ferric-browser-storage -p ferric-browser-engine-qt
  -p ferric-browser --all-targets --locked -- -D warnings`.

## Limitations

Migrations beyond version 1 and permission workers remain later slices.
Kernel-backed crash-safe lock recovery, write-time retention pruning, command-history capping,
clean-break schema refusal and checksum validation are covered;
long-running retention scheduling and repair/export remain future work.
Corruption and future-schema refusal plus read-only inspection are
covered separately in `docs/testing/m2-15-storage-recovery.md`. Download-index
writes currently occur on the Qt/Rust UI thread. High-volume committed-
navigation history writes use the acknowledgement-backed batch path described
in `docs/testing/m0-109-history-write-worker.md`. The
switcher's history/bookmark/quickmark/download reads and external
`downloads.query`, read-only history/bookmark/quickmark commands, permission
queries/decisions, and the site-status ledger now use the bounded metadata
worker described in `docs/testing/m0-104-storage-worker.md`; direct
  library-command writes, journey queries/writes, migration work, and session
  enumeration/writes remain synchronous pending later worker requests.
