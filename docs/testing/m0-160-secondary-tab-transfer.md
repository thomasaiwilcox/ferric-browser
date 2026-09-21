# M0-160 — Secondary-window live-tab source path

Secondary windows now participate in the same live transfer boundary as the
primary window. The primary IPC poller resolves the focused registered window,
requests its typed transfer payload, and moves the actual secondary
`WebEngineView`. Existing secondary views expose dynamic ownership metadata so
their dialog, popup, permission, profile, and interceptor handlers follow the
view when it is reparented.

After a successful transfer, the source completes `TransferTabOut`, creates a
new blank fallback tab, and keeps the application usable. Failed target
creation or adoption restores the detached source view. The destination may be
the primary pool, an existing secondary window, or a newly-created same-profile
window.

Evidence:

- `cargo fmt --all`
- `cargo test -p ferric-browser-engine-qt tab_give_command_preserves_target_and_queues_live_transfer -- --nocapture` — passed
- `cargo xtask check --locked` — passed (all workspace unit and doc tests)
- `cargo build -p ferric-browser --locked` — passed
- Native Wayland run with `--basedir /tmp/ferric-browser-m0-160-verify2.2BYMFE`:
  - Created a same-profile secondary window.
  - `tab-detach` created a new window from the focused secondary source.
  - `tab-give <primary-owner-token>` moved the live source view into the
    primary window without navigation; the primary then reported two live
    tabs and the source registry entry retained a fallback tab.
  - Runtime output contained no transfer QML error, layout warning, or
    compositor warning.
