# M4-03 Hyprland boundary evidence

Date: 2026-09-20  
Status: in-progress

## Task card

- Requirements: HYPR-001, HYPR-002, and the workspace-routing portion of
  HYPR-003.
- Files: `crates/browser-engine-qt/src/hyprland.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: the browser discovers the current Hyprland instance from
  the runtime environment, keeps request and event sockets separate, exposes a
  stable RustBrowser desktop-ID client query, and routes a validated context
  workspace only when `hyprland.enabled` is `on` or `auto` with
  `workspace_routing = true`.

## Safety boundaries

The adapter never writes compositor configuration. Missing sockets, malformed
responses, denied requests, and stale settings are reported through the normal
browser status path and do not prevent Qt window activation. Workspace values
are bounded and reject whitespace and command separators before being placed in
the documented Lua dispatcher request used by Hyprland 0.55+ (`hl.dsp.focus`
and `hl.dsp.window.move`), while retaining the legacy dispatcher form for
0.51–0.54 after a bounded `j/version` probe. Selectors are JSON-escaped before
entering the Lua request. Client discovery matches the stable
desktop ID, not a title or PID alone; rows without a bounded, hexadecimal
Hyprland client address are discarded. Request connections retry transient
compositor restart errors with a
short capped backoff and fail immediately for non-transient permission errors.
Workspace route/move requests and browser-client discovery use the bounded
single-flight Hyprland worker; Qt invokables perform only bounded validation and
return the last sanitized client snapshot while discovery is pending.

Context workspace routing is additionally gated on the owning Qt window being
registered and actually active. A denied or pending compositor activation cannot
cause a workspace request to target whichever other window happens to be active.
Validated route and move requests are submitted to a bounded single-flight
worker; the Qt thread only performs configuration/status checks and polls the
result, so compositor socket timeouts and restart backoff cannot block the UI.
Window movement adds a compositor-side identity guard: after Qt requests the
captured window's activation, it queries `j/activewindow` and refuses the move
unless the mapped active client has a valid opaque address and the stable
RustBrowser class/initial-class identity. This avoids moving an unrelated app
when activation is denied or the target disappears. The public Qt 6 Wayland
API still does not expose a per-window Hyprland address directly, so exact
multi-window movement remains dependent on the compositor accepting the Qt
activation handoff.

The optional Omarchy package carries a version-qualified, commented example
rule and keybinding. Users must explicitly copy/include and review them;
RustBrowser never edits `hyprland.conf` or installs global bindings.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p browser-engine-qt --locked --offline`
- `cargo build --locked`
- `cargo xtask check`
- `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The focused adapter suite covers disabled mode, selector validation, request
socket behavior, and stable desktop-ID filtering. Live compositor routing and
per-window address mapping remain qualification work; ambiguous compositor
ownership must fail closed rather than move the wrong window. On 2026-09-20,
the live Hyprland 0.56.2 session accepted a configured `workspace = "3"`
context route and switched the active workspace to 3 without blocking the
browser. The client remained on its original workspace, so exact-window
placement/movement is still not claimed as qualified.
