# M0-98: clean native Wayland startup

Requirements: ENGINE-001, UI-001, UI-007, and STATE-009

The repository harness is now designed to launch the built application under
a disposable Weston headless Wayland compositor, a private `XDG_RUNTIME_DIR`,
and a fresh `dbus-run-session`:

```text
cargo xtask test wayland
```

The harness waits for the nested compositor socket, keeps RustBrowser alive for
the bounded 10-second smoke window, reports any early exit with captured output,
and cleans up both child processes and the private runtime directory.

GUI startup now checks Qt's selected QPA after `QGuiApplication` creation. The
`xcb` and `xwayland` backends fail closed with an explicit unsupported-platform
error; `offscreen` remains available for the headless adapter smoke. An
explicit `RUSTBROWSER_ALLOW_NON_WAYLAND=1` override is accepted only for
development diagnostics and emits a warning; it never qualifies native
Wayland behavior.

Desktop portal capability probing is asynchronous and session-global. File
selection honors `desktop.portals = required`: it waits for the bounded probe
and cancels with a concrete status when the portal service is unavailable.
`desktop.portals = auto` retains the Qt native-dialog fallback. Screen capture
checks the specific `ScreenCast` interface in required mode and cancels with a
concrete status when it is unavailable; it never falls back to unauthorized
capture. Confirmed external URI and completed-download opens likewise check
the separate `OpenURI` capability in required mode before invoking the desktop
handler. Web notifications check the separate `Notification` capability in
required mode and report cancellation when it is missing.

On the current machine (`2026-09-20`), `weston` is available at
`/usr/bin/weston` and:

```text
cargo xtask test wayland
```

completed successfully. The run built RustBrowser, launched the repository's
bounded loopback fixture server, opened its `/editable` page, then launched a
disposable headless Weston compositor with a private runtime directory and
D-Bus session. The fixture is one-request bounded, and the harness requires
that the browser actually reaches it before success. It kept the application
alive for the bounded 10-second lifetime and removed the runtime including
stale `doc`/`gvfs` portal mounts.
The harness also attempts a real editable-page input check when the optional
`wtype` helper is installed: it asks `wtype` to
replace the focused textarea contents and waits for the fixture's
`/input-result` request. The installed Weston headless compositor does not
advertise `zwp_virtual_keyboard_v1`, so this input check is reported explicitly
as `not-run`; missing `wtype` is reported the same way. Neither result is
counted as a pass or as a browser failure. The run
therefore qualifies native Wayland startup, fixture-backed launch, and clean
process teardown only; interactive input, focus, and portal interaction remain
open. A fresh direct-session run on
2026-09-19 used the live Hyprland session with
`QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`;
it reached the timeout (`124`) with no startup or QML error output.

On 2026-09-20, the direct-session smoke also exposed and then verified a
startup handoff fix: `main.rs` supplies profile runtime overrides through
`startupProfileOverridesJson`, and `Main.qml` now declares and applies that
property before profile bootstrap. A bounded rerun no longer reports Qt's
initial-property warning for this handoff.

The same bounded direct-session smoke on 2026-09-20 also no longer reports the
previous invalid `Keys` attachment or Fusion dialog implicit-width binding
loop; the popup handler now owns keyboard focus through its `contentItem`, and
the userscript-removal dialog has an explicit bounded width.

Interactive tab, popup, close, and discarded-page reload qualification still
require a native UI harness and fixture.
