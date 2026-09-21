# M0-159 — Same-profile live-tab give

The Qt adapter now publishes its live browser-window registry through
`windows.query`. Each entry includes the local core window ID and a globally
unique owner token; `tab-give` accepts either identifier and resolves the target
against the primary QML registry. The primary view pool then reparents the
existing `WebEngineView` into an already-open same-profile secondary window.

The destination adopts the typed transfer metadata without navigating. The
source completes `TransferTabOut` and keeps a blank fallback tab, while a
rejected or stale target restores the source view to its original pool.

Evidence:

- `cargo fmt --all`
- `cargo test -p browser-engine-qt tab_give_command_preserves_target_and_queues_live_transfer -- --nocapture` — passed
- `cargo build -p rustbrowser --locked` — passed
- Native Wayland run with `--basedir /tmp/rustbrowser-m0-159-registry.5cGdR4`:
  - `query windows --format json` exposed two live registry entries with
    distinct owner tokens.
  - `command -- tab-give <secondary-owner-token>` was accepted.
  - The source query retained one blank fallback tab after the transfer, with
    no navigation or transfer error in the native log.
