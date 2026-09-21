# ADR-0004: Browser-owned permission and capture authority

- Status: accepted for the V1 implementation boundary; live portal
  qualification pending
- Date: 2026-09-18
- Scope: engine permission requests, trusted prompts, WebRTC capture, and
  desktop portal ownership

## Decision

RustBrowser keeps permission authority in the browser-owned Rust/Qt adapter.
Engine requests enter a per-window bounded prompt queue, are grouped by exact
origin and capability, and are cancelled on navigation, tab/window teardown,
or expiry. Durable rules and session grants use the profile/session identity
boundary; private sessions never persist decisions. Desktop-media selection
is explicit and system portals remain the authority for source consent,
camera, microphone, and screen/window capture.

The browser does not implement a parallel capture protocol, auto-accept
requests, log credentials, or treat a local fixture as proof of a portal
session. Missing portal/PipeWire capability is reported explicitly.

## Consequences and evidence

This preserves Chromium and compositor security boundaries while giving the
user an inspectable prompt lifecycle. Real portal consent, device switching,
and Google Meet qualification remain release evidence gates.

- `crates/browser-engine-qt/src/lib.rs`
- `crates/browser-engine-qt/qml/Main.qml`
- `crates/browser-storage/src/permissions.rs`
- `docs/testing/m3-01-permissions.md`
- `docs/testing/m0-15-portal-probes.md`
- `docs/testing/m0-36-media-capture.md`

## Reconsideration trigger

Revisit only if the pinned Qt public API cannot represent a required request
or a portal protocol changes its authority model; do not add private Qt APIs as
a compatibility shortcut.
