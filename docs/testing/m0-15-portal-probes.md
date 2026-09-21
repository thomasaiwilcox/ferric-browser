# Portal and PipeWire probe evidence

Date: 2026-09-19  
Status: in-progress

## Scope

The shared CLI/IPC diagnostics snapshot now performs bounded, read-only runtime
probes. It introspects the user D-Bus portal object with `busctl` and reports
FileChooser, ScreenCast, OpenURI, and Notification independently, including
their advertised interface versions. It probes PipeWire's core with `pw-cli`
and reports the observed core version. Each probe is capped at 750 ms and 64
KiB of output; it never includes bus output, account names, hostnames, URLs, or
paths in the diagnostics payload.

Missing probe tools, an unavailable user bus, an absent portal interface, and a
failed PipeWire core probe are represented as explicit unavailable or
not-tested facts with remediation-oriented reasons. No service is restarted and
no machine configuration is written.

The diagnostics unit suite also checks that a valid advertised version is
reported as available while missing and non-numeric interface versions remain
unavailable, preventing required-mode UI from treating malformed introspection
as a usable portal.

This slice establishes capability detection only. File selection, ScreenCast
consent/session requests, PipeWire stream ownership, and native WebEngine media
qualification still need the corresponding asynchronous integration and live
Wayland evidence.

The current development session also produced this bounded diagnostics result:

- Qt/QtWebEngine: 6.11.2; CXX-Qt: 0.10.0;
- FileChooser v4, ScreenCast v6, OpenURI v5, and Notification v1 advertised;
- PipeWire core 1.6.8 observed;
- native Wayland and Hyprland runtime facts observed;
- codecs, DRM, WebAuthn transports, system-audio capture, notification
  delivery, and Chromium security-patch attribution remain `not-tested`.

This output is environment evidence, not live consent or device qualification.

## Verification

```text
cargo test -p ferric-browser-engine-qt --locked --offline
cargo clippy -p ferric-browser-engine-qt --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo xtask check
cargo run -p ferric-browser --locked --offline -- diagnostics --format json
```
