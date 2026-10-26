# Everyday credential boundary: Qt transport proof

Status: prototype evidence only; the phone credential provider remains disabled.

## Scope

T046 exercises Qt 6.11.2 WebEngine/WebChannel using a disposable HTTPS server,
a self-signed certificate accepted only by the test page, a harmless native
`record(QString)` probe, and hostile page/frame fixtures. It contains no
authenticator material and adds no production WebChannel wiring. The existing
native WebAuthn path and the production no-WebChannel guard remain unchanged.

The test reserves Qt script world 42 after source-auditing the application for
world IDs 0, 1, and 2 and finding no existing use of 42. Its test-only
world-42 mediator sends `location.origin`, `isSecureContext`, and a
document-creation nonce through the channel to a harmless native receiver.
T046 reads the corresponding browser-calculated facts in world 42 and checks
that the native receiver received those exact values. The MainWorld fixture
claims a different origin, replaces `qt`, poisons its own prototype, and calls
its fake transport; the claim does not match or alter the native receiver's
origin. The nonce remains stable for a same-document fragment navigation and
changes on reload, with the reloaded value observed by the native receiver.

Test scripts model hostile MainWorld and ApplicationWorld code in the main
document and child frames. They check for a transport in their own worlds and,
if present, attempt a harmless channel call with forged values. T046 asserts
that these attempts do not reach the native receiver. A world-42 inspection
of child-frame transport presence is diagnostic only: calling
`runJavaScript(..., 42)` on a child is native test instrumentation, not a
page-originated attacker action. The fixture's `document.domain` assignment is
recorded as hostile behavior; this test does not prove cross-origin
relaxation.

## Public API boundary

Qt 6.11.2 `QWebEngineFrame` exposes frame validity, child enumeration,
`isMainFrame()`, `url()`, and frame-scoped JavaScript execution. It does not
expose a native security-origin property or stable document identifier.
`QWebEnginePage::setWebChannel(channel, worldId)` selects the JavaScript world
for the channel transport, and Qt allows one channel per page. The
`QWebEngineScript` subframe option controls script injection; it is not proof
that other frames cannot reach a channel transport.

Accordingly, `isMainFrame()`, `isValid()`, and the frame URL checked by T046
are native frame-handle facts; URL is routing context and is not
security-origin proof. The isolated world's `location.origin` and
`isSecureContext` are browser-calculated JavaScript facts sent by the mediator;
they are not exposed as native `QWebEngineFrame` facts. The document-creation
nonce is test-world evidence that distinguishes the current fixture document
and reload. It is not a Qt-native document ID or a production generation
lease. T047 owns transition and stale-reply coverage.

## Gate and limitation

T046 exercises a positive main-frame channel call and asserts that hostile
MainWorld/ApplicationWorld attempts do not alter or reach the native probe.
The child world-42 transport read remains diagnostic; WebChannel does not
provide caller-frame identity, so this test does not claim that an isolated
child caller can be distinguished from the main frame.

T047 uses a separate disposable page and a test-only pending completion. It
captures a native frame handle, a reserved-world document nonce and a native
generation advanced by actual page load-start events. Completion runs through
Qt's asynchronous frame-scoped JavaScript callback against the current main
frame; admission requires the captured source to be main-frame, the generation
and nonce to match, and the current browser-computed origin/security context to
match the test's trusted fixture. The current-document control must pass;
same-URL reload and child-frame completions must fail. The test enables Qt
6.11.2's BackForwardCache setting on its disposable page and requires
`pageshow.persisted` on history restoration. It also requires the restored
document to retain its nonce while the native generation advances, so the
queued pre-navigation completion is rejected by generation rather than by
nonce inequality or a reload-shaped substitute.

T047's source frame is held by native test instrumentation. It does not claim
that WebChannel identifies a caller frame, that the nonce is a Qt-native
document ID, or that the test's generation gate is production ownership code.
On the exact Qt 6.11.2 run, T046 and T047 both passed with zero skips. The
Back traversal reported `loadFinished(false)` while the page URL returned to
the original document; the isolated-world probe independently observed
`pageshow.persisted == true`, the preserved document nonce, an advanced native
load-start generation, and an empty `notRestoredReasons` list. T047 then
rejected the pre-navigation completion and admitted a fresh post-restore
completion. The `pageshow.persisted` and nonce/generation assertions establish
the BFCache outcome; `loadFinished(false)` is recorded as observed Qt signal
behavior rather than treated as the BFCache proof.

Even passing T046 and T047 does not establish a native security-origin API or
a production document-ownership lease; those gaps remain explicit for later
production design. No MainWorld native object, console bridge, custom scheme,
or CSP bypass is an acceptable fallback.

References: [QWebEngineFrame](https://doc.qt.io/qt-6.11/qwebengineframe.html),
[QWebEnginePage](https://doc.qt.io/qt-6.11/qwebenginepage.html),
[QWebEngineScript](https://doc.qt.io/qt-6.11/qwebenginescript.html).
