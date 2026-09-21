# M0-163 UI automation boundary

Date: 2026-09-20  
Status: in-progress

The production boundary is checked by \`cargo xtask check\`. It scans the Qt
adapter, QML, launcher, and packaging inputs for Chromium remote-debugging
configuration and automation-socket markers. The check fails closed if such a
marker is added to a production path.

The repeatable adapter integration smoke is \`cargo xtask test adapter\`. It
builds the application, launches the Qt/QML bridge with the offscreen QPA and
\`--temp-basedir\`, runs for a bounded ten seconds, rejects early QML/module
failure, and always terminates the disposable process. It does not create an
automation endpoint or inspect a page through CDP.

This is a boundary check, not a replacement for native input qualification.
The nested Wayland harness additionally attempts a focused editable-page input
check through `wtype`; on the current Weston headless build it reports input as
`not-run` because `zwp_virtual_keyboard_v1` is not advertised. Public Qt input,
dead keys and IME composition, focus recovery, accessibility hooks, portal
behavior, popup adoption, and real Wayland interaction still require a native
desktop test environment. Those limitations are retained in TEST-005 evidence
instead of being inferred from the offscreen smoke.

## Validation

- \`cargo xtask check --locked --offline\`
- \`cargo xtask test adapter\`
- workspace strict Clippy and formatting checks
