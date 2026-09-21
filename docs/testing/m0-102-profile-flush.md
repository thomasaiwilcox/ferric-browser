# M0-102: durable profile flush ordering

Requirement: STATE-009

Normal profile metadata uses an explicit `ProfileStore::flush()` boundary. The
boundary performs a full SQLite WAL checkpoint and fails if the checkpoint is
busy, so an orderly close does not claim that Rust-owned metadata is durable
while another profile connection still holds the WAL open. The storage test
writes a visit, checkpoints it, closes the store, and verifies the record after
reopen.

The Qt adapter exposes `flush_durable_state()`. For normal profiles it first
writes the final `last-session` snapshot using the existing atomic file-plus-
directory fsync path, then checkpoints the profile database and clears the
session checkpoint's dirty state. Private and ephemeral profiles return success
without creating durable state.

The process-wide QML coordinator calls this operation after all secondary and
popup participants have closed and before the root reducer receives
`RequestShutdown`. A secondary window performs the same final snapshot/database
ordering before acknowledging its own close. A failed flush leaves the window
open with a storage-specific retry/keep-open surface and exposes the force-quit
recovery path; forced quit preserves the crash marker. Renderer-helper teardown
and native multiwindow flush-latency qualification remain open.

Evidence for this slice:

- `cargo test -p ferric-browser-storage --locked --offline`
- `cargo test -p ferric-browser-engine-qt --locked --offline`
- `cargo build -p ferric-browser --locked`
- `cargo fmt --all -- --check`
- native Wayland startup smoke with `QT_QPA_PLATFORM=wayland`
