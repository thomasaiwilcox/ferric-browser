# M0-23 diagnostics surface evidence

Status: in progress, implementation slice recorded 2026-09-16.

## Scope

The browser now exposes the existing privacy-safe diagnostics snapshot through
a browser-owned Qt Quick surface. The native adapter reuses the same
`diagnostics.get` assembly used by IPC, including protocol and crash-marker
fields, and bounds the displayed text to 48 KiB. The surface is read-only,
supports refresh and copy, has an explicit dialog role/name, takes focus while
visible, and closes through Escape or a named action while restoring the
captured browser target.

The snapshot remains conservative: probe failures are represented as
unavailable/not-tested facts, and the UI does not expose arbitrary page content
or a privileged WebChannel.

## Verification

```text
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The runtime smoke is expected to end with timeout status 124 after staying
alive for the full interval. Interactive copy behavior and assistive-technology
traversal remain manual follow-up work.
