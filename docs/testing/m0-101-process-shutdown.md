# M0-101: process-wide shutdown coordination

Requirement: STATE-009

The primary window now coordinates shutdown across registered secondary windows
and popup windows. It first completes the primary page/download checks, enables
the Rust IPC mutation gate, asks every child surface to resolve its own active
work and page-state prompts, and waits for child destruction before requesting
the root reducer shutdown and calling `Qt.quit()`.

The coordinator exposes the current stage while it waits. After ten seconds it
offers an explicit force-quit action. That action records a forced shutdown in
the Qt adapter; the application bootstrap then leaves the durable crash marker
in place so the next launch can offer recovery. Read-only IPC remains available
while mutating command/action/activation requests receive bounded `E_BUSY`
responses. The active-work decision cancels browser-owned file, permission,
dialog, certificate, WebAuthn, desktop-media, and capture requests before
retrying the participant close.

Static Qt coverage, workspace tests, the application build, and native Wayland
startup smoke cover the implementation. Native multiwindow interaction,
renderer-helper teardown, and forced-exit qualification remain qualification
work; the durable profile flush ordering is covered by M0-102.
