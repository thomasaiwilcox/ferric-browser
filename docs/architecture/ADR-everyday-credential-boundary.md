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

## T048 — Selected public host route and remaining proof

**Decision:** continue on Qt's public Qt 6.11.2 `QWebEngineView` / its
`QWebEnginePage`; do not fork Qt WebEngine. The production design is a
page-bound, credential-only `QWebChannel` installed in one centrally reserved
isolated script world, with a browser-owned mediator
injected for the top-level document and a private native receiver owned by that
page's host. The channel carries only an untrusted, bounded notification that a
request may be pending; its payload carries no authoritative origin, target,
profile, tab, frame, or result destination. Native code must freshly challenge
the current main-frame mediator and retrieve the private pending request from
that mediator before admission. It then binds the request to the native page,
profile, host, and document epoch, and revalidates those bindings at
completion. Keep the channel object and its lifetime tied to that page. Qt
documents one channel per page, and setting a channel in another world replaces
the installed channel; the production integration therefore needs one
deliberate page/world choice, not a second generic bridge. Publish only the
narrow credential request API.

The page's current `mainFrame()` and `QWebEngineFrame` validity, main-frame
flag, URL, and frame-scoped `runJavaScript` provide useful routing and live
frame handles. They do not supply a native security origin or a stable
cross-document identifier. Treat frame URL only as routing context. The
mediator may report browser-computed origin, secure-context, and
permissions-policy facts from its own isolated world, but those remain
browser-side JavaScript facts: they must be challenged against the current
main frame and corroborated by native page, profile, host, and document
incarnation state at both admission and completion. A nonce is an equality
check within the mediator protocol, not native document identity or a lease.
The T046/T047 prototype proves only its isolated fixture behavior, including
stale rejection after reload and BFCache restoration; it does not prove this
production mediator, receiver, or lease.

**Provider choice and activation remain unqualified.** The public page/channel
route can carry a bounded request to a native host, but the Qt APIs reviewed
here expose no documented native user-gesture token for that request and do
not establish that transient page activation survives an asynchronous
mediator/channel round trip. `webAuthUxRequested` and
`QWebEngineWebAuthUxRequest` describe user-verification, resident-account, and
failure UX for an authenticator that already needs interaction. They are not a
phone-provider chooser or evidence that the page request followed a physical
user action. A message's `isTrusted` value is not such evidence either.

Keep provider selection in an application-owned native UI and keep the phone
provider disabled until a downstream production test proves the positive native
provider-choice/consent path and its cancellation or skipped-choice rejection.
The test must reject synthetic dispatched transport messages and other-frame
submissions, while binding the private request to the same live main-frame
document, page/profile, and host through completion. Ordinary programmatic
requests from scripts in the page's own realm are not categorically rejected:
same-origin scripts share that page authority under C04. Do not assume that
transient page activation survives the asynchronous mediator/channel round
trip, and do not infer a physical gesture from message `isTrusted`. The native
provider-choice path must be exercised on the actual application UI. If the
required document binding or native choice/consent path cannot be proved with
public APIs, stop and leave phone unavailable; do not add private Qt hooks,
infer authority from URL/channel delivery, or weaken the native-authentication
path.

Before production wiring, independently prove: one per-page channel and
reserved world with no unintended page/profile or child-frame access; the
mediator's own top-level-window message checks against forged, dispatched, and
child-frame messages; native same-URL reload, navigation, BFCache, renderer,
view, profile, and window invalidation; exact HTTPS/top-level origin and
permissions-policy decisions; and a real native provider-choice positive
consent and negative cancel/skipped-choice path. Replace the old blanket source
guard only through reviewed production isolation tests. Until those results and the independent review pass, the prototype is
feasibility evidence only and no real credential provider may be wired.

**Qt 6.11.2 public references:**
[QWebEnginePage](https://doc.qt.io/qt-6.11/qwebenginepage.html) documents
`mainFrame()`, per-world JavaScript and WebChannel installation, and
`webAuthUxRequested`; [QWebEngineFrame](https://doc.qt.io/qt-6.11/qwebengineframe.html)
documents frame validity, main-frame status, URL and frame-scoped JavaScript;
[QWebEngineScript](https://doc.qt.io/qt-6.11/qwebenginescript.html) documents
isolated worlds and script injection; [QWebChannel](https://doc.qt.io/qt-6.11/qwebchannel.html)
documents that registered QObject properties and public methods are exposed to
remote HTML clients; [QWebEngineWebAuthUxRequest](https://doc.qt.io/qt-6.11/qwebenginewebauthuxrequest.html)
documents the narrower authenticator-interaction UX lifecycle.
