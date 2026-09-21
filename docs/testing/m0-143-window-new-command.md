# M0-143 `window-new` command

`window-new` now has one shared registry definition and typed CLI/IPC handling for:

- `window-new`
- `window-new --profile NAME`
- `window-new --private`
- the combined profile/private form

The engine validates an explicitly selected durable profile through the
profile-list worker before queuing a window action. The command returns an
accepted pending response while that bounded lookup is polled. Without an
explicit profile, normal windows inherit the active
named profile; private windows from a transient profile use a dedicated
private name. The existing QML window component consumes the action and creates
a blank secondary Wayland toplevel with the requested profile and privacy mode.

Verification:

- `cargo fmt --all -- --check`
- `cargo test -p browser-core --locked --offline` (57 passed)
- `cargo test -p rustbrowser --locked --offline` (37 passed)
- `cargo test -p browser-engine-qt --locked --offline` (108 passed)
- `cargo xtask check --locked`
- native Wayland startup smoke with `--temp-basedir`
