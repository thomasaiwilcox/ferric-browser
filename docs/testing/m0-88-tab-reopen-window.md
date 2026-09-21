# M0-88 — Explicit state-loss window reopen boundary

The browser now exposes `reopen-in-window` and the typed
`browser.tab.reopen-window` action. It validates the current live tab and
retains only a sanitized safe URL descriptor before creating a same-profile
native window. The result and status explicitly warn that live page state,
forms, media, and in-progress engine work are not preserved.

The typed action and browser-owned toolbar/switcher paths now stop at an
accessible confirmation dialog before consuming the reopen payload. Escape and
Cancel discard the pending operation; only explicit Reopen converts the
confirmation marker into the bounded window-open effect. IPC receives the same
pending-confirmation result and the active browser surface owns the consent
step.

`tab-detach` and `browser.tab.detach` now use the supported live-view transfer
boundary. The command queues the browser-owned detach action, and the QML host
creates a same-profile destination while transferring the existing
`WebEngineView` without navigation. The transfer path is bounded by the
existing stable tab payload and source rollback checks; forms/media and
graphics qualification remain tracked in TAB-003.

The native window path carries normal/private/ephemeral profile identity and
shares only the existing off-the-record profile for transient owners. URL
payloads are bounded by the existing safe URL policy and are consumed by both
primary and secondary QML window hosts.

Evidence:

- `cargo test -p browser-core -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked`
- `cargo fmt --all -- --check`
