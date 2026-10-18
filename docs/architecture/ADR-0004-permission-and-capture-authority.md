# ADR-0004: Browser-owned permission and capture authority

- Status: accepted for the V1 implementation boundary; live portal
  qualification pending
- Date: 2026-09-18
- Scope: engine permission requests, trusted prompts, WebRTC capture, and
  desktop portal ownership

## Decision

Ferric Browser keeps permission authority in the browser-owned Rust/Qt adapter.
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

- `crates/ferric-browser-engine-qt/src/lib.rs`
- `crates/ferric-browser-engine-qt/qml/Main.qml`
- `crates/ferric-browser-storage/src/permissions.rs`
- `docs/testing/m3-01-permissions.md`
- `docs/testing/m0-15-portal-probes.md`
- `docs/testing/m0-36-media-capture.md`

## Reconsideration trigger

Revisit only if the pinned Qt public API cannot represent a required request
or a portal protocol changes its authority model; do not add private Qt APIs as
a compatibility shortcut.

## Notification authorization and presentation

Ferric's Qt profiles use `AskEveryTime`, keeping saved/session permission policy
and explicit consent in the application permission-request handler. Qt delivers
a notification to the presenter after that authorization. The presenter must
not start a second saved-permission wait that can cancel a just-approved
request when the metadata store is unavailable. A known Deny, explicit
revocation, disabled desktop notifications, or a retired profile/window still
blocks delivery. Required desktop-service discovery and transport remain
separate checks and cannot fall back to another transport.

This follows the pinned
[Qt 6.11 permission-persistence contract](https://doc.qt.io/qt-6.11/qwebengineprofile.html#PersistentPermissionsPolicy-enum).
It does not treat an unavailable saved choice as permission: the Qt callback
already carries an authorized notification. There is no extra remembered grant
or independent permission cache in the presentation controller.

A normal-mode, private-bus fixture reproduces the former failure using a real
Qt permission prompt and an invalid stored decision for its exact loopback
origin. The native regression covers healthy and unavailable saved policy,
a saved Deny, and denial before the desktop service acknowledges delivery.
These fixtures do not prove the separate shared-profile source-window
attribution or physical desktop notification interaction requirements.

## File chooser cancellation and window parenting

Qt 6.11.2's desktop portal file dialog does not send `Request.Close` when the
application hides or retires it. A private-bus test reproduced this with the
actual Qt platform theme. File selection therefore uses a browser-owned Qt
D-Bus request actor, with the public KWindowSystem 6.28
`KWaylandExtras::exportToplevel()` API for the requesting Wayland window.
KWindowSystem owns its platform implementation; browser code includes no
private Qt headers. The window export remains cached for the native window's
lifetime, so one request cannot revoke another request's shared parent handle.

Each picker has its own bus connection, pinned portal service owner, request
identity, and bounded transport/cleanup deadlines. User decision time is not
limited. Retiring the page, owner, or controller closes an outstanding request,
including a request whose method reply arrives after cancellation. Response
subscriptions are installed before sending the picker request. Unknown and
unsupported file locations fail as a whole, without delivering a partial
selection or being mislabeled as user cancellation.

Auto mode can use the owned widget chooser when parent export or capability
discovery fails before a picker request is sent. Required mode reports that
failure. Once the request has been sent, an uncertain outcome never opens a
second picker. The widget fallback explicitly disables Qt platform dialogs.
Capture remains under the existing Chromium/portal authority described above.

`tests/portal` verifies the production controller against isolated D-Bus and
Wayland fixtures, including actual native window/process identity. This does
not establish real desktop backend focus, physical input, or capture consent;
those remain separate release qualification requirements.
