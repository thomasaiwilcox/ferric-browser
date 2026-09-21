# M2-03 session evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: SESSION-001 through SESSION-004 (storage/session slice).
- Files: `crates/browser-storage/src/sessions.rs`,
  `crates/browser-storage/src/lib.rs`, `crates/browser-engine-qt/src/lib.rs`,
  its QML lifecycle, workspace `Cargo.toml`/`Cargo.lock`, and
  `docs/requirements.csv`.
- Observable result: `SessionSnapshot` serializes bounded, schema-versioned
  session/profile UUIDs, revision/time, ordered windows, selected tabs, pin/
  mute/zoom state, and optional scroll position. Construction omits private
  tabs and never stores their URLs. Only explicitly safe HTTP(S), file, or
  about descriptors are replayable as GET; POST/unknown/auth/blob/data
  navigation becomes a URL-free placeholder. `restore_plan` validates the
  structure and returns the selected tab before lazy secondary entries.
  `save_session_atomic` validates, writes with mode 0600 where supported,
  fsyncs, retains one prior generation beside the destination, renames in
  place, and fsyncs the parent directory; loading falls back to that prior
  generation when the current file is unavailable.
  The Qt bridge now builds a normal-profile `last-session` snapshot from live
  tabs and atomically checkpoints it when the owning view closes; private
  profiles have no session path. The application now creates a durable
  no-browsing-data crash marker before GUI startup, surfaces a recovery-
  available indicator after an unclean prior run, and removes the marker only
  after a clean GUI return. Core mutations mark the session dirty; a 50 ms
  QML timer lets Rust debounce writes by 500 ms while enforcing a five-second
  maximum checkpoint interval. Recovery restores the selected tab first and
  keeps secondary recovered tabs at `about:blank` until selected.
  Named sessions are profile-scoped and support validated `session-save NAME`,
  `session-load [--append] NAME`, and `session-list` commands; the QML model
  consumes the restore payload and preserves the selected appended tab. Exact
  deletion is exposed only through a confirmation-gated bridge method.
  The native QML session manager lists profile-local sessions, requests its
  validated restore plan through the storage worker, and polls the bounded
  result without scanning or parsing session files on the GUI thread. Actual
  Replace/Append loading consumes that same worker result and applies only the
  validated descriptors. It requires an explicit Replace or Append choice,
  handles stale/invalid files in the confirmation surface, and uses a
  two-step exact-name delete confirmation, with deletion acknowledged by the
  storage worker before the list is refreshed. The command-session preview and
  load keep the same browser-owned validation boundary.
  Crash-recovery checkpoint discovery and parsing use the same worker handoff;
  the recovery banner remains responsive while checkpoint generations load.
  Normal startup cleanup of stale checkpoint generations is also acknowledged
  by the worker before recovery data is considered removed.
  Loaded tab scroll positions are sampled asynchronously by QML, persisted in
  the next durable checkpoint, and applied after a safe session restore.
  Normal-profile checkpoints also carry a bounded, URL-safe list of recently
  closed descriptors. Crash recovery restores that list only after the same
  worker-side snapshot validation; private descriptors are never serialized,
  and the configured undo limit is clamped to the specification's 100-entry
  profile bound.
  The atomic session writer also has a bounded fault-injection seam in its
  storage tests. Interruptions after the temporary-file fsync, previous-
  generation rename, current-generation rename, and directory fsync are
  checked for cleanup, recoverability, and generation selection.
- Evidence: `cargo xtask test storage` (64 storage tests), `cargo test
  -p browser-storage -p browser-engine-qt --locked
  --offline` (64 storage tests and 167 Qt-engine tests), `cargo test
  --workspace --locked`, and
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.

## Limitations

Full multi-window capture, unsaved-page/download checks, real
process-kill/disk-failure qualification, and filesystem-full qualification
remain later work. A
pending selected-tab restore now blocks
checkpoint replacement until that navigation completes; failed restores leave
the prior checkpoint intact.
