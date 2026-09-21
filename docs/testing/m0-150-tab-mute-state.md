# M0-150 — Native tab-mute state

Completed the tab-mute command contract across the core, Qt adapter, and
WebEngine view.

- The Qt command result now reports the requested muted state instead of its
  inverse.
- Accepted mute changes produce a bounded pending action carrying the stable
  tab ID and boolean state.
- QML binds each WebEngineView's `audioMuted` property to the tab model and
  applies pending IPC actions to that model.
- The tab strip and session restore continue to expose the same durable mute
  flag.

Evidence:

- cargo fmt --all -- --check
- cargo test -p ferric-browser-engine-qt --locked --offline
- cargo xtask check --locked
- Wayland startup smoke with --temp-basedir
