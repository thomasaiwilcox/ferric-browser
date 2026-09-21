# M0-141 profile-delete command

Date: 2026-09-17  
Status: verified

## Scope

`profile-delete NAME` now uses the same preview and confirmation boundary from
the profile manager when invoked through interactive command dispatch or typed
IPC. Rust validates the exact durable profile name and emits a bounded action
only after a preview can be generated; QML presents the exact UUID-owned Rust
data, session, and cache roots and requires the existing explicit confirmation
button before deletion.

The deletion implementation still refuses the active profile and any profile
held by another live window/process. Private profiles cannot target durable
profile deletion, and no command path deletes QtWebEngine storage.

Rust-owned profile directories are first renamed to transaction-specific sibling
tombstones. The registry removal is then durably committed before tombstone cleanup.
Startup recovery restores staged directories when the registry entry remains, or
finishes cleanup when deletion was committed. Cleanup trouble after commit is reported
as pending cleanup rather than incorrectly reporting that the profile deletion failed.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-core --locked --offline` — 57 tests
- `cargo test -p ferric-browser --locked --offline` — 35 tests
- `cargo test -p ferric-browser-engine-qt --locked --offline` — 106 tests
