# Everyday credential boundary: Qt transport proof

Status: opt-in production integration under validation; no real-provider/device qualification. The dated prototype decisions below are historical; the latest integration checkpoint records current evidence.

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

## Production receiver implementation — 29 October 2026

The page-owned receiver is now being implemented in `credential_boundary.{h,cpp}`
with a credential-only notification object in world 42. Channel messages carry
no authoritative payload. Native code freshly challenges the actual main-frame
mediator and binds its response to both native incarnation and captured Rust
target identity. Provider choice is an owned native dialog; success completion
requires a provider selection, while cancellation may terminate before selection.
This implementation does not yet connect a real provider or qualify native
authenticator fallback/activation. Independent review and physical qualification
remain required before phone enablement.

The bundled `qml/scripts/credential/qwebchannel.js` is copied verbatim from the
qualified Qt 6.11.2 SDK, preserving its SPDX notice. Its SHA-256 is `bb21d1663083c3fefed95b895282d095aaa1f57f7697c0170ea79ffd205421b2`.
The client and mediator are embedded by the existing browser-resource manifest.
The general QML no-WebChannel guard remains in place.

The MainWorld adapter in `qml/scripts/credential/shim.js` is a candidate installed
only by the isolated native test fixture. It snapshots BufferSource views by
byte offset/length, retains native request methods for fallback, reconstructs
credential/response interfaces with weakly held records, and permits ordinary
instance/prototype method overrides. Capability functions remain untouched.
Conditional, platform-only, enterprise-attestation and policy-denied requests
remain native. This candidate does not authorize helper launch or establish
native activation/physical qualification. The actor/provider wiring remains
pending independent review.

### Scoped independent review — 29 October 2026

The user-authorized independent reviewer closed the boundary/helper baseline
review with no outstanding findings. The review found and prompted fixes for
unbounded authenticator assertion retention, resident-key legacy-field
precedence, a completion/probe race that stranded future requests, and admitted
blob extensions whose results the library did not preserve. Assertions now have
fixed count and cumulative CTAP-payload bounds; unsupported blob requests remain
native and are rejected by the helper before transport.

The reviewer independently reran 17 helper library and four binary tests, and
578 upstream library tests passed with one existing ignored test. The scoped
native Wayland run (`quick-review-r5`) passed six Qt cases with no skips,
including actual renderer termination, distinct profile replacement, policy
denial, channel replacement, skipped-choice rejection, chooser cancellation and
probe/completion overlap. The external build directory retains source hashes,
logs and owned-compositor cleanup receipts.

This closes the baseline review prerequisite for integration. Native/Core
binding, process-wide ceremony ownership, effective network admission, selected
account delivery, native fallback and physical-device verification still need
their integrated checks. It does not enable the production provider.


### Production association foundation — 29 October 2026

The independent reviewer closed the native/Core production-association and
staged-window foundation review with no outstanding findings. The actual
profile lifetime supplies a native-only current-view snapshot, including its
Qt profile, actual window, owner Ui and incarnations. The adapter corroborates
Core tab/index and registered prompt identity and replaces invalid bindings.
Background documents receive the isolated mediator before navigation; enabling
it in an existing HTTPS main document uses a guarded, operation-stamped bounded
installation. Credential admission still requires the active Core tab and
actual native foreground window.

Request validity includes window context, configuration and installed-policy
epochs. The actual profile interceptor has a nonwrapping generation and one
native current owner per profile, confined to the application thread. A
competing interceptor requires explicit detachment; historical owners cannot
claim attachment or clear a successor. A sibling window's installed-policy
change revokes an existing request even when its requesting Ui has not updated.

Owned Wayland `quick-association-r8` passes twelve Qt cases with zero failures
or skips. `page-association-r8` passes six production-association and thirteen
window cases with zero failures/skips; cleanup receipts pass. The unique-profile
fixture now keeps profile factories alive through view disposal. This closes
foundation findings only. Provider/actor/MainWorld-shim wiring, effective direct
network admission, account/result delivery, integrated regression/packaging and
physical-device verification remain required before release.


### Window/session integration checkpoint — 30 October 2026

Independent review closed the scoped native command and session findings.
Constructing an unadopted child view cannot overwrite its opener's prompt
registration. Queued selection continuations retain the captured Core Target,
profile/privacy/context/group scope and active-tab identity, checked after Qt
selection/mode/repeat notifications. Native-owned errors report revoked work.
IPC rejects context selectors for these selection commands, returns the actual
Core WindowId and completes asynchronous operations on native receipts.

Session snapshots aggregate actual normal-profile owners. Restore metadata is
staging-only; a presented window's live context takes precedence. Shared path
ordering is captured before independent worker backlogs and rejects delayed old
writes while preserving valid durable generations. Schema 5 reads older files.

Fresh-archive owned Wayland results are association 6/6, window 18/18 and Quick
boundary 12/12, with zero failures/skips and clean teardown (`r23`). Actual IPC
has four passing production cases; component tests pass 148 application and
115 storage cases, with warnings-denied Clippy. Native CMake harnesses now check
source freshness and track archive content to force relinking after Cargo
promotes an older-timestamp archive. These checks caught and repaired a stale
harness build; stale runs do not qualify new source.

This is an engineering checkpoint. Broader window behavior, compositor
integration, production actor/provider/MainWorld wiring, effective direct
network admission, account/result delivery and physical qualification remain
unfinished. No real credential or release approval is claimed.

### Native chain and cancellation review — 30 October 2026

Actual Qt foreground/background tab requests inherit their opener's logical
Rust group; native windows and dialogs retain independent grouping. Literal CLI
chains use one bounded typed envelope, decoded completely before execution.
One process-local receipt follows direct selection hops, preserving the captured
document and original issuer. Only the terminal owner queues focus/completion;
discarding a queued target callback fails the original operation. Nested repeat,
macro and dynamic command/action executors remain explicitly unavailable in
window-mode chains until they can preserve this receipt. Recorded multi-selection
playback is also rejected before effects. This is a documented parity gap.

Close attempts revoke credentials before QML close guards run. Cancellation may
delete the native adapter or host synchronously; guarded event filtering consumes
that event rather than forwarding it to a deleted receiver. Linux helper startup
arms parent-death SIGKILL and checks the captured parent PID before exec. A real
exec'd fixture verifies the armed signal and process termination after owner death.

Independent review closed the reported synchronous-close teardown and multi-hop
focus/completion findings, including the nested-workflow bypass. Evidence:
`page-chain-supervision-r32` passes association 6/6, window 26/26 and supervisor/
parent-death 3/3 each; `quick-close-r28` passes 15/15; `window-ipc-r31` passes ten
production cases. Native sessions/private buses clean up, and engine/browser
Clippy passes with warnings denied. Provider/MainWorld wiring, broader window
workflow parity and physical qualification remain required; no real credential
or combined-release approval is claimed.

### Peer page-state mutation review — 30 October 2026

Pin/mute commands now resolve actual registered owners within the same logical
group and profile/privacy/context scope. Their queued application checks native
owner, page generation and current group membership. The return queue captures
the issuing page, requiring its exact active Core Target before document work.
Pure mutation keeps focus; an inherited selection receipt retains final focus
intent. Both queues retain the original receipt through destruction or failure.

Independent scoped review closed with no findings. `page-mutations-r34` passes
association 6/6 and window 29/29; `window-ipc-r34` passes fourteen production IPC
cases. No failures/skips or residual owned processes are reported, and engine
Clippy passes with warnings denied. Global ordering, broader commands/sessions
and credential provider/device work remain incomplete.

### Global page-order review — 30 October 2026

Logical page order is reserved before native materialization and kept separately
from owner registration order. Moves stay within a pinned partition and retain
actual page/view identity. Fallback native owners synchronize their Core order
through validated ReorderTabs, preserving active/previous selection and document
identity. Pin/unpin also synchronizes this order. Retirement, transfer and transient
owner release forget removed IDs. Aggregate sessions retain canonical order;
selected-first loading and independent recovery roots keep original positions.

Independent scoped review closed. Fresh native `page-order-r40` passes association
6/6 and window 32/32; production IPC passes 17 cases. Owned sessions/private buses
close cleanly. Core 174/174, storage 117/117 and three focused engine order/cleanup
unit tests pass. Workspace formatting and warnings-denied engine/Core/storage/
runtime/application Clippy pass (`r41`/`r42`). Detach/give, close/undo, nested
workflows, compositor integration and production credential/device work remain.

### Scoped window Undo and optional tagging — 30 October 2026

Closed-page recovery now survives an ordinary window's retirement in a bounded
process store keyed to actual ProfileInstanceId, privacy and context. Its current
group is preferred; staged unpresented pages retain their saved context/group.
Only approved closes record safe descriptors. Private/ephemeral records disappear
when their final ordinary owner closes, even if the empty primary Ui remains as
an application service controller. Unowned transient records cannot be reinserted
by the record-after-transfer path. Normal records are bounded to 100 per profile
and 1000 process-wide, and persist only through normal-profile snapshots.

A profile-scoped reservation precedes reopen effects and Qt property publication.
It prevents duplicate reentrant consumption and releases on early return; dropping
it cannot resurrect erased private records. Shared record/remove/import changes
invalidate peer caches and checkpoints across all contexts in the actual profile,
with session rechecks on delivery. Schema 6 preserves closed context/group and
reads 2–5. Reopen creates a fresh document with saved settings and position, not
saved DOM state. Empty startup model rows adopt Core pin/mute/zoom before views
are created, preserving actual native zoom through prepared-window restoration.

Extended recovery tests exposed an independent portability crash: KF 6.28's
xdg-toplevel tag callback dereferences an unavailable manager on a compositor
without that global. The application now observes the actual Qt connection using
public QWaylandClientExtension before installing that callback. Observation scans
Qt's existing globals, binds no protocol resource and adds no blocking round trip
([Qt 6.11.2 source](https://raw.githubusercontent.com/qt/qtbase/v6.11.2/src/plugins/platforms/wayland/global/qwaylandclientextension.cpp)).
Identity preparation and tag-request admission are distinct. Unsupported servers
keep usable untagged windows. Admission is not proof of observed compositor binding
or grouping; those still require exact native correlation and separate validation.

Independent scoped source review closed. Current evidence: association 6/6,
window 38/38, session recovery 10/10 and last-tab undo 41/41 (`r55`); tab recovery
44/44 (`r58`, correcting an old schema-4 assertion to 6); checkpoint recovery 4/4
(`r57`); production IPC 17 cases (`r56`). Native sessions/private buses close
cleanly. Engine units pass 562 with one existing opt-in performance test ignored;
five Undo units and storage 118/118 pass. Engine/storage warnings-denied Clippy
and workspace formatting pass. Broader window commands/workflows, compositor
grouping and integrated credential/provider/device work remain unfinished.

### Process-wide credential integration in progress — 30 October 2026

The private native binding now feeds one QApplication-owned ceremony actor.
It carries the opaque Core/native lease, allocates its helper correlation ID,
checks single-flight/capacity/deadline/phase fences, and starts only the fixed
sibling helper after native phone choice and effective network admission.
Results remain private and bounded until a clean supervised exit; trailing
output or a failed exit cannot qualify an apparent success. Native delivery
rechecks its monotonic deadline, lease and policy after the asynchronous
main-frame challenge. Bounded native-only account labels are separate from
the WebAuthn credential JSON; ambiguous multiple results are declined.

Opt-in MainWorld installation preserves original-method native fallback and
removes listeners/restores methods on disable without replacing the document.
The installed interceptor's relay deny/host rules remain effective. General
ABP paths without an existing site bypass, unresolved system proxies and
unsupported routing decline phone use rather than bypass policy.

Scoped review identified a missing native TLS fact: secure-context JavaScript
does not prove absence of certificate exceptions. Qt caches exceptions by host,
certificate and error bits, across view/port changes
([exact Qt 6.11.2 source](https://raw.githubusercontent.com/qt/qtwebengine/v6.11.2/src/core/net/ssl_host_state_delegate_qt.cpp)).
A private native profile factory now establishes TLS observation at actual
profile birth before publishing it; importing an existing profile cannot assert
freshness. Every managed view is observed before navigation through native
adoption, independently of the phone switch. Host taint is bounded, never evicted
while that actual profile survives, and overflow/unknown history deny admission.
Native loading/frame/renderer facts and unsupported TLS flags complete the
predicate. This factory/TLS increment is awaiting its native regression run and
review closure; its implementation is not yet qualification evidence.

Before that TLS increment, `actor-native-r61` passes Quick 16/16 with zero skips
and clean owned-session/private-bus teardown; `actor-units-r61` passes 3/3.
Helper `r62` passes 17 library and 4 binary cases; warnings-denied all-target
helper Clippy passes `r64`. Consent-qualified excluded-credential errors,
synthetic-helper end-to-end delivery, integrated security closure and actual
device qualification remain unfinished. No real credentials were connected.


### Integrated synthetic delivery and teardown — 30 October 2026

Fresh-profile TLS regressions now pass: trusted profile CA, ordinary HTTPS
401/404 documents, accepted bad-certificate rejection, cross-port exception
reuse and destroyed unobserved views. Imported profiles remain ineligible;
only the native factory establishes fresh observation before publishing a
new actual profile. The owned Quick run passes 28 cases after adding callback
deletion regressions. Profile lifetime's standalone cases pass 11 + 3.

The production actor/shim path passes 18 synthetic-helper scenarios (Qt 20/20,
`actor-pipeline-native-r78`), including Get/Create interface reconstruction and
final exclusion delivery as InvalidStateError. Output waits for clean helper
exit; late, stale, duplicate-progress, trailing-output and nonzero-exit cases
fail safely. Abort, policy change, rejected close, disable, global busy and
synchronous integration destruction also pass. The fixed-sibling fixture has
no network/Bluetooth or real credential signatures. Natural helper reaping,
layout removal and private bus/session teardown are verified. Four runner
ownership regressions (`r79`) cover normal exit, timeout, SIGTERM and malformed
receipts; forced cleanup is separately recorded and cannot qualify a pass.

The helper maps excluded credentials only at the final awaited CTAP2
MakeCredential stage with a nonempty exclusion list. CTAP 2.2 permits this
response after a presence timeout, so it does not assert positive user presence.
Silent preflight, early GetInfo/PIN, Get and empty-list failures remain generic.
Unsupported appidExclude stays native. Helper tests pass 18 + 4 and upstream
582 + one existing ignored test; source hashes and exact patch round-trip pass.

Callback/cleanup review findings are resolved. Operation errors now reject
any implemented annotation; admission errors require exactly false. Focused
validation of that stricter decoder and native routing is pending. Native
bootstrap snapshots Qt-effective Chromium flags before profiles; direct
admission refuses changed flags, proxies and unknown routes. A single exact
captured-RP-host-to-127.0.0.1 rule is the only admitted resolver override, never
one for an assigned relay. The routing increment is under independent review.
No real credential or physical-device outcome is claimed.


### Reviewed integration and device-test readiness — 30 October 2026

Independent review has no retained production credential or fixture finding.
Explicit `--direct-network` is a fresh-process, whole-browser route selection;
proxy conflicts and existing-instance forwarding are rejected. It does not
change route as a side effect of choosing a phone. Native `r90` passes six CTest
suites: Qt startup policy 8/8, Quick 29/29, ordinary/mapped helper pipelines
21/21 each, runner ownership 4/4 and Widget shim 3/3. Both pipelines exercise
explicit direct bootstrap and retain clean exit, deadline and delivery fences.
Workspace all-target warnings-denied Clippy passes `r96`; CLI/application/engine
units pass 119/150/567, with the existing opt-in benchmark ignored.

The actual production Root admits Phone at the exact disposable HTTPS RP with
profile-only fixture trust and scoped loopback mapping (`r99`). Its automatic
check rejects the chooser without selecting the helper or creating credentials.
The real helper and PSL are copied into an owned installation prefix, separate
from any installed browser. No personal profile, global CA or DNS setting changes.
The helper's disposable installation check passes. RP verifier and lifetime tests
pass 16/16: stalled accepted TLS handshakes cannot hold shutdown; browser cleanup
precedes RP stop; interruptions and cleanup errors fail the run while retaining
failed ownership evidence. RP counters record only independently verified results
and never automatically claim a physical-device outcome. The attended iPhone
registration/assertion and phone/provider deletion remain to perform.
