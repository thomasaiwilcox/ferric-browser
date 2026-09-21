# M0-145 — Typed direct in-page search

The shared command registry now exposes `search TEXT` as a direct in-page
search command. The command accepts bounded query text, optional backward
direction, and an explicit `smart`, `sensitive`, or `insensitive` case mode.

The CLI and typed IPC surface both validate those fields before producing the
same parsed command. The Qt engine validates the command again, resolves the
current live tab, and sends `Event::StartSearch` through the reducer. The
reducer retains the query, direction, and case mode, so `search-next` continues
the same search without losing its matching policy. Repeat eligibility includes
`search` while preserving the existing count and nesting limits.

Native primary and secondary WebEngine views receive a bounded tab-delimited
find action. Backward traversal uses `FindBackward`; sensitive searches use
`FindCaseSensitively`; insensitive searches retain Qt's default matching
behavior. Smart searches use case-sensitive matching when the query contains
an uppercase Unicode code point and case-insensitive matching otherwise. No
page script or WebChannel is used for this browser-owned operation.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p browser-core --locked --offline` — 57 passed
- `cargo test -p rustbrowser --locked --offline` — 39 passed
- `cargo test -p browser-engine-qt --locked --offline` — 110 passed
- `cargo xtask check --locked`
- Wayland startup smoke with `--temp-basedir` — expected timeout, no error,
  panic, failed, or assert diagnostics
