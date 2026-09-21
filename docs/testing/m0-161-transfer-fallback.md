# M0-161 — Secondary transfer fallback accounting

Secondary-origin live-tab transfer no longer creates two reducer fallback tabs
in the source window. `complete_tab_transfer()` already creates the mandatory
blank tab when the transferred tab was the source's last tab; the QML source
completion path now creates only the corresponding fallback `WebEngineView`.

Evidence:

- `cargo fmt --all`
- `cargo test -p browser-engine-qt --lib tab_give_command_preserves_target_and_queues_live_transfer --locked` — passed
- `cargo build -p rustbrowser --locked` — passed
- Native Wayland run with `--basedir /tmp/rustbrowser-m0-161-registry.QfGADs`:
  - Created a same-profile secondary, detached its live view, and gave the
    secondary-origin view to the primary by owner token.
  - `query windows --format json` reported registry counts of `2`, `1`, and
    `1` for the primary and two secondary windows respectively.
  - `query tabs --format json` reported two healthy primary tabs, with no
    transfer, layout, anchor, or compositor errors in the native log.
