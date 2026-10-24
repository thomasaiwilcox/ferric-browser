# Everyday credential boundary: Qt transport proof

Status: prototype evidence only; the phone credential provider remains disabled.

## Scope

T046 exercises Qt 6.11.2 WebEngine/WebChannel using a disposable HTTPS server,
a self-signed certificate accepted only by the test page, a harmless native
`record(QString)` probe, and hostile page/frame fixtures. It contains no
authenticator material and adds no production WebChannel wiring. The existing
native WebAuthn path and the production no-WebChannel guard remain unchanged.

The test reserves Qt script world 42 after source-auditing the application for
world IDs 0, 1, and 2 and finding no existing use of 42. It attaches the
test-only channel to that world, injects the Qt WebChannel JavaScript helper
only into the main frame, verifies a top-frame method call, and probes both
child frames for direct transport access and method invocation. MainWorld and
ApplicationWorld checks exercise page replacement of `qt`, a forged origin
claim, page prototype poisoning, a fake transport call, and `document.domain`.
The fixtures' `document.domain` assignment is recorded as hostile behavior;
this test does not claim to prove cross-origin relaxation behavior.

## Public API boundary

Qt 6.11.2 `QWebEngineFrame` exposes frame validity, child enumeration,
`isMainFrame()`, `url()`, and frame-scoped JavaScript execution. It does not
expose a native security-origin property or stable document identifier.
`QWebEnginePage::setWebChannel(channel, worldId)` selects the JavaScript world
for the channel transport, and Qt allows one channel per page. The
`QWebEngineScript` subframe option controls script injection; it is not proof
that other frames cannot reach a channel transport.

Accordingly, `isMainFrame()` and the frame URL checked by T046 are native frame
facts, but URL is routing context and is not security-origin proof. The
isolated world's `location.origin` and `isSecureContext` are browser-calculated
JavaScript facts observed through frame-scoped execution; they are not exposed
as native `QWebEngineFrame` facts. The document-creation nonce is test-world
evidence that distinguishes the current fixture document and reload. It is not
a Qt-native document ID or a production generation lease. T047 owns transition
and stale-reply coverage.

## Gate and limitation

T046 asserts that child frames cannot read or use the world-42 transport. A
child that reaches the harmless probe makes the test fail and records a failed
isolation primitive. Until the exact Qt 6.11.2 runtime result is reviewed, the
prototype is unqualified. Even a passing transport test does not establish a
native security-origin API or a production document-ownership lease; those
gaps must remain explicit in any later production design. No MainWorld native
object, console bridge, custom scheme, or CSP bypass is an acceptable fallback.

References: [QWebEngineFrame](https://doc.qt.io/qt-6.11/qwebengineframe.html),
[QWebEnginePage](https://doc.qt.io/qt-6.11/qwebenginepage.html),
[QWebEngineScript](https://doc.qt.io/qt-6.11/qwebenginescript.html).
