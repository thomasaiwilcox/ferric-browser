# M0-144 `tab-give` boundary

`tab-give WINDOW_ID` is part of the shared command registry and has typed
CLI/IPC argument handling. The engine validates that exactly one bounded target
window ID is supplied and queues a browser-owned transfer request. QML resolves
the target by the live window registry, requires a different same-profile
window, detaches the existing WebEngineView, adopts the typed metadata, and
only then completes the source reducer transfer. Rejection restores the source
view. `reopen-in-window` remains the explicit safe-URL/state-loss fallback for
cases where live transfer cannot be completed.

Verification:

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-core --locked --offline` (57 passed)
- `cargo test -p ferric-browser --locked --offline` (38 passed)
- `cargo test -p ferric-browser-engine-qt --locked --offline` (109 passed)
- `cargo xtask check --locked`
- native Wayland startup smoke with `--temp-basedir`

Live forms/media preservation and graphics validation across primary-to-
secondary and secondary-origin transfers remain required qualification.
