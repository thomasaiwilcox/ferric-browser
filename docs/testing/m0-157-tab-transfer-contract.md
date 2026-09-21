# M0-157 — Typed live-tab transfer contract

Implemented the Qt-independent state boundary required by TAB-003:

- `Event::TransferTabOut` removes a live tab from its source window without
  emitting `CloseView`, navigation, or reload effects.
- `Event::TransferTabIn` adopts URL, title, loading state, pinned state, mute
  state, and bounded zoom metadata into a destination window, reusing its
  initial blank tab when appropriate.
- The Qt bridge exposes bounded JSON payload/adoption/release methods for the
  forthcoming view-reparent path and rejects profile-name mismatches.

The primary-window QML adapter now has the persistent view pool and both
`tab-detach` and exact-target `tab-give` reparent paths. Secondary windows keep
the transfer source ownership until the destination has adopted the typed
metadata; fallback-view allocation is preflighted before secondary-source
completion, while failed adoption or source completion rolls back the
destination reducer state and restores the source view. A successful final
source transfer consumes the prepared fallback (or creates the primary
fallback) without leaving a half-completed source state.

Evidence:

- `cargo test -p browser-core transfer_` — passed (2 tests)
- `cargo check -p browser-engine-qt` — passed
- `cargo clippy -p browser-engine-qt --all-targets --locked --offline -- -D warnings` — passed
- `cargo xtask test adapter` — passed (bounded offscreen Qt/QML smoke)
- Qt compiler emitted the existing upstream header warnings; no new errors.
