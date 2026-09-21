# M2-17 Site Doctor experiment boundary evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: SEC-007.
- Files: `crates/browser-engine-qt/src/lib.rs` and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: Site Doctor experiments are explicit, visible, scoped to
  one captured tab/document, automatically expire, and can be individually
  undone without weakening unrelated security boundaries.

## Evidence

- The supported experiments are blocker bypass, matching-userscript disable,
  compiled-default site settings, and a fresh same-profile view. Private sessions, missing HTTP(S) origins,
  duplicate bypasses, and unsupported experiment names are rejected.
- Each experiment carries a generated ID, exact origin, safe URL, captured tab,
  and optional temporary tab. The Site Ledger and status payload expose the
  active kind, target, and remaining lifetime while it is active.
- Experiments expire after 30 seconds through a Rust monotonic deadline checked
  by the live QML heartbeat. Expiry uses the same cleanup path as
  `site-doctor-undo ID` and restores temporary blocker/userscript state or
  closes the temporary view.
- Navigation to another document, tab/view close, and profile reinitialization
  invalidate the experiment. A successful blocker result is only a pending,
  separately confirmed host-scoped runtime proposal; userscript and fresh-view
  results do not persist compatibility changes.
- No experiment changes TLS/HSTS, grants permissions, clears site/profile data,
  disables the renderer sandbox, changes the global proxy, or crosses profile
  boundaries.
- `cargo xtask check` passes, including workspace unit and doc tests. Strict
  workspace Clippy and a native Wayland startup smoke also pass.

## Limitations

The pinned Qt adapter still does not expose a direct active-stream/device
probe. Full multi-window/profile-shutdown qualification and live
per-experiment two-document fixtures remain future work.
