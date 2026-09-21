# M2-04 tab operations evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: the tab ordering/movement portions of TAB-002 and TAB-003.
- Files: `crates/ferric-browser-core/src/model.rs`,
  `crates/ferric-browser-core/src/reducer.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, and
  `crates/ferric-browser-storage/src/sessions.rs`.
- Observable result: live tabs carry pinned, muted, and bounded zoom state;
  pinning preserves a pinned-first order, movement stays within the pinned or
  unpinned group, same-profile movement remains reducer-owned, and the QML tab
  strip exposes pin, mute, and left/right movement controls. The shared
  `browser.tab.pin` and `browser.tab.mute` actions, plus the corresponding
  `tab-pin TAB_ID [on|off|toggle]` and `tab-mute TAB_ID [on|off|toggle]`
  commands, route tab state changes through the same reducer boundary. Session snapshots
  preserve these tab properties and restore payloads carry them into the QML
  model.
- Evidence: `cargo test --workspace --locked --offline` (39 core tests and
  15 storage tests), `cargo clippy --workspace --all-targets --locked
  --offline -- -D warnings`, `cargo build -p ferric-browser --locked --offline`,
  and a timed native Wayland launch with `--temp-basedir`.

## Limitations

Cross-window drag/drop, explicit window-level movement UI, and scroll-position
capture remain later slices. User-facing zoom controls are available in the
primary and secondary chrome, and navigation-scoped content settings are
snapshotted per live document so reloads do not mutate an already-loaded page.
The current
profile manager opens a selected profile in a new window; full profile
switching and lifecycle integration remain outstanding.
