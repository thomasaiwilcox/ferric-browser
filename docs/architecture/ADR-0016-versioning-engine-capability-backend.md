# ADR-0016: Candidate engine backend for strict Versioned Browsing guarantees

- Status: owner approved the maintained pinned-backend direction on 2026-10-08; implementation and release qualification incomplete; no production capability enabled
- Date: 2026-10-08 (updates the 2026-09-29/30 research decision and 2026-10-07 checkpoint)
- Scope: per-view WebRTC activity and exact, partition-aware cookie snapshot/seed/merge

## Context

ADR-0015 requires native evidence before a strict capability is enabled and explicitly rules out treating a remote-debugging endpoint or private engine API as an implicit workaround. The installed Qt WebEngine 6.11.2 public surface failed two independent qualifications:

- A connected, data-only WebRTC channel remained open while the hidden view's `recommendedState` was `Frozen`; it requested no media permission. Neither lifecycle nor permission state proves the source's `live_call: Clear` category ([VB-033](../testing/versioned-browsing/VB-033.md)).
- The public cookie callback loses partition identity, did not round-trip tested non-default SameSite values, and provides neither an enumeration-complete boundary nor an exclusive mutation boundary ([VB-005](../testing/versioned-browsing/VB-005.md), [cookie API gap](../testing/versioned-browsing/cookie-api-gap.md)). The earlier claim that host-only provenance is necessarily lost was too strong: the pinned canonical domain's leading-dot convention survives Qt's conversion. It still needs a correctly qualified reconstruction recipe.

These are capability failures, not missing timers. Until the contract below is proved, the current fail-closed behavior, clean branches and separately qualified structural work remain the product behavior.

## Decision under review

Do **not** use Qt WebEngine remote debugging / CDP as the production backend for either strict guarantee. Pursue a **version-pinned, maintained Qt WebEngine/Chromium native extension** as the full-vision backend direction, with independently certifiable activity, snapshot/seed and merge interfaces. On 2026-09-30 the project owner authorized the isolated prototype and documentation updates. On 2026-10-08 the owner additionally approved pursuing the shipping-backend direction and its maintenance/security work. This authorizes engineering toward a reviewable backend, **not** release of the current diagnostic engine or promotion of any unqualified capability. A WebRTC-only extension must not imply cookie support.

| Route | Per-view live-call proof | Cookie identity and read-back | Consistent source snapshot / exclusive apply | Disposition |
| --- | --- | --- | --- | --- |
| Current public Qt adapter | Negative counterexample | Lossy on tested build | Not exposed | Keep fail-closed |
| CDP through Qt remote debugging | No qualified PeerConnection/data-channel absence signal in the documented domains | Richer cookie fields, including partition key, but opaque keys and source provenance still need qualification | `Storage.getCookies` and `setCookies` expose no versioned snapshot, compare-and-swap or atomic batch contract | Reject as production strict backend; disposable research only |
| Native engine extension | Positive observation and scoped frozen-registry witness passed, including OOP frames and freeze-handler microtasks; retained admission **not qualified** | Synthetic 14-record and real HTTPS 19-record full-member copies/request checks passed; legacy/expiry/fault qualification remains | Existing single-sequence `GetAllCookies` is a promising snapshot boundary; live merge separately needs conditional/exclusive mutation control across all writers | Isolated native baselines passed; broader qualification, integration and security review remain |

The CDP distinction matters: `Storage.getCookies` is documented as returning all browser cookies, and `Storage.setCookies` takes an array, but neither description promises an atomic point-in-time read, an all-or-nothing write, isolation from responses/service workers, or a source revision. `Network.getCookies` is URL-scoped, so it is not a substitute. The protocol cookie type carries `partitionKey` and `partitionKeyOpaque`, but not an explicit host-only provenance field. These are observations about the *documented contract*, not proof that no Chromium implementation could ever add a stronger operation. [CDP Storage](https://chromedevtools.github.io/devtools-protocol/tot/Storage/), [CDP Network](https://chromedevtools.github.io/devtools-protocol/tot/Network/).

Enabling Qt remote debugging would also expose a powerful control surface for an ordinary profile. Qt documents a host/port setting and says that, absent an explicit `--remote-allow-origins`, it adds `--remote-allow-origins=*`. Loopback binding alone does not authenticate other local processes or prove safe target/profile mapping. Do not start such an endpoint against a real profile for this feature or its tests. [Qt WebEngine debugging](https://doc.qt.io/qt-6/qtwebengine-debugging.html).

## Candidate interface and ownership

1. **Activity:** The browser process reports a bounded, content-free state for the exact view and its entire live frame tree: `Active`, `Clear` or `Unknown`, with a view-generation token. It includes data-only PeerConnections, camera/microphone capture and screen capture. Creation, connection-state changes, frame swaps, crashes, listener gaps and teardown invalidate `Clear` until reconciled. Never forward URLs, SDP, ICE candidates, device labels, stats, data-channel messages or media. The pinned Chromium source also exposes `WebContentsObserver::OnFrameIsCapturingMediaStreamChanged(RenderFrameHost*, bool)` and `WebContents::ForEachRenderFrameHost`; the latter covers frames in the WebContents, including BFCache and prerendered pages. `RenderFrameHostImpl::HasMediaStreams(kCapturingMediaStream)` offers an internal snapshot query, but it is not on the public `RenderFrameHost` interface. These are a plausible patched-engine reconciliation/invalidation route, not yet a tested Qt bridge or race-free absence proof. [Chromium observer](https://chromium.googlesource.com/chromium/src/+/HEAD/content/public/browser/web_contents_observer.h), [WebContents frame traversal](https://chromium.googlesource.com/chromium/src/+/HEAD/content/public/browser/web_contents.h), [pinned implementation source](https://chromium.googlesource.com/chromium/src/+/HEAD/content/browser/renderer_host/render_frame_host_impl.h).
2. **Cookies:** The network-service side returns complete canonical records for one validated profile/scope, including partition identity and all required attributes, together with an engine-issued completion/boundary certificate. The bridge never reads SQLite profile files. Destination seeding is restricted to a new, non-navigated isolated profile, uses in-memory secret buffers, checks individual outcomes and reads back before navigation. A live-profile merge requires a conditional apply protocol that checks a source revision and excludes *every* competing cookie writer (including HTTP responses, service workers, scripts and other tabs) for the apply interval, or returns `Unsupported`/`Unknown` without claiming success. Chromium's published `CookieManager` has asynchronous get/set/delete operations and canonical-cookie types; those calls alone are not a transaction or writer lock. [Chromium cookie manager interface](https://chromium.googlesource.com/chromium/src/+/HEAD/services/network/public/mojom/cookie_manager.mojom), [Chromium cookie overview](https://chromium.googlesource.com/chromium/src/+/HEAD/net/cookies/).
3. **Application boundary:** The new adapter may supply evidence to the existing `CookieBackendCapabilities` contract and stash admission. It cannot construct a positive capability merely because a callback or list was observed. The application and reducer retain owner/generation checks, native consent, cancellation and partial/unknown outcomes. No cookie value is logged, durably stored as a versioning object, or returned to QML.

## Security and maintenance conditions

- The extension is compiled into a pinned engine build, not reached through a listening debugger, generic local socket or page JavaScript. Only the owning Ferric process can invoke it; profile/view handles are opaque and revalidated at the engine boundary. Renderer-originated messages are not authority to request snapshots or set `Clear`.
- Cap requests and result sizes; bound callback lifetimes; zero or promptly discard secret buffers; avoid raw values in traces, crash diagnostics and test output. Treat opaque/unrepresentable partition keys and unsupported attributes as `Unknown`, never silently discard them.
- Do not disable the renderer or network-service sandbox to simplify the bridge. Review the Qt/Chromium patch, build reproducibility, CVE update cadence, distribution/licence notices and rollback plan separately before any fork is adopted ([ADR-0010](ADR-0010-license-and-release-identity.md)).
- A backend certificate is tied to exact Qt/Chromium revision, patch revision, OS/platform, profile mode, scope, required identity dimensions and test evidence. An engine update invalidates it until requalified. The default public-Qt backend remains available and fail-closed.

## Feasibility gates, in order

1. **No-profile prototype:** Pin the source/build fingerprint and locate the Qt-to-Chromium ownership seams, starting at Qt 6.11.2's [`WebContentsAdapter`](https://github.com/qt/qtwebengine/blob/v6.11.2/src/core/web_contents_adapter.cpp) and profile adapter. Build an isolated, reproducible patched engine using disposable off-the-record profiles and synthetic origins only. Qt's [source-build requirements](https://doc.qt.io/qt-6/qtwebengine-platform-notes.html#building-qt-webengine-from-source) include a C++20 toolchain and the Chromium build dependencies; the installed public-header package alone is not a source tree. Stop if the fork cannot be maintained and sandboxed within project resources.
2. **Activity proof:** Map all frames, including out-of-process frames, to one view; subscribe before claiming `Clear`; reconcile on frame/view/process lifecycle and missed events. Run a connected data-only channel plus camera, microphone and screen-capture counterexamples. Race a new connection against admission and the final pre-freeze recheck. Any unresolved interval is `Unknown` and refuses the stash.
3. **Cookie representation proof:** Round-trip host-only/domain, path, expiry/session, Secure/HttpOnly, Strict/Lax/None/unspecified SameSite, partitioned/unpartitioned and opaque-key cases in a disposable profile. Compare actual request behavior and canonical read-back; reject unsupported records rather than narrowing identity invisibly.
4. **Snapshot/seed proof:** Demonstrate an engine-issued complete read boundary under concurrent response, script and service-worker writes; seed a fresh child and verify before its first navigation. A quiet period, repeated equal lists, frozen views or Ferric-side revision is not a boundary.
5. **Merge proof:** Demonstrate conditional live-profile application under concurrent writers, interruption, process loss and partial failure. If the engine cannot prevent or detect those races, keep `cookie_merge_apply` disabled even if snapshot/seed qualifies. Never advertise whole-jar rollback unless the engine actually provides it.
6. **Integration and release:** Run the existing end-to-end slice and final campaign suites on the pinned native build, perform security and licence review, and only then update capability evidence and user-facing claims. Each capability is promoted separately; no gate changes as a consequence of this ADR alone.

## Pinned source inspection — 2026-09-29

Read-only inspection of Qt WebEngine tag `v6.11.2`, commit
`a33fa2a897e5ee58e385b3f88dc247d99fca56db`, confirms the following adapter seams.
Its `src/3rdparty` gitlink pins Chromium integration commit
`5170777d28bee1ce92cc693a0dbf2ad01492e5cf`. The sparse source checkout is under
ignored `target/versioning-engine-spike/qtwebengine`; no engine patch, Chromium
submodule checkout, engine build, or production private-API dependency was added.

- `WebContentsAdapter::determineRecommendedState` checks visibility, loading,
  audibility, related active contents, inspector state, audio/video/mirroring/
  desktop capture and crashes before recommending Frozen. It does not check
  PeerConnections. This explains why the native data-only-channel counterexample
  cannot be repaired by treating that recommendation as an absence certificate.
  [Pinned implementation](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/web_contents_adapter.cpp#L2125).
- `CookieMonsterDelegateQt::getAllCookies` passes an empty completion callback
  to the network service. `setCookie` similarly discards the individual mutation
  result; its canonical-cookie construction supplies no partition key. Adding
  application timers cannot recover these discarded engine results.
  [Pinned delegate](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/net/cookie_monster_delegate_qt.cpp#L89).
- `toQt(const net::CanonicalCookie&)` copies name/value, domain, expiration,
  HttpOnly, path and Secure, but not SameSite or partition identity. This is a
  concrete explanation for the observed **callback representation loss**, not
  evidence that Chromium itself forgot the stored SameSite attribute.
  [Pinned conversion](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/type_conversion.h#L226).

Exposing the complete canonical representation and completion callbacks is a
bounded adapter-extension candidate. Further pinned Chromium inspection found
that `GetAllCookiesAsync` funnels through `DoCookieCallback`; after pending store
loading, `GetAllCookies` collects ordinary and partitioned cookies, copies them
and invokes its callback on the cookie-store sequence. The global-operation
queue is specifically designed to preserve consistent export/import reads.
This is a plausible existing point-in-time boundary, not proof that a new
multi-writer transaction is necessary just to take a snapshot. Persistent-load,
expiry, concurrent-writer, owner-loss and Qt bridge tests must still qualify it.
[Pinned CookieMonster implementation](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_monster.cc),
[queue contract](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_monster.h).

`CanonicalCookie::Domain()` documents the leading-dot host/domain distinction;
Qt copies that domain without adding a dot in its callback conversion. Do not
confuse that with the public **setter**, which can broaden a non-empty domain.
The canonical path also preserves SameSite, partition site/ancestor/nonce,
source scheme/port/type, priority and timestamps that the public callback omits.
[Pinned cookie identity](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_base.h),
[canonical comparison](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/canonical_cookie.cc).

Live-profile merge remains a different problem. The proposal permits explicit
partial/unknown outcomes; it does not require an invented all-or-nothing
rollback. It does require the preconditions, writer control, exact outcomes and
read-back promised by the approved merge contract. Deprecated
`SetAllCookiesAsync` is not a safe shortcut: it replaces ordinary cookies and
does not supply those guarantees.

## Isolated prototype — 2026-09-30

[Probe source, patch and runner](../../tools/versioning-engine-spike/README.md)
and the [evidence/continuation record](../testing/versioned-browsing/engine-capability-spike.md)
are now the concrete next-work entry points. The prototype contains:

- A synthetic-only canonical cookie round trip using real `GetAllCookies` and
  per-record `SetCanonicalCookie` callbacks, full-member comparisons, partition
  collision cases, nonced-key refusal, lifetime guards and a bounded deadline.
- An observation-only, bounded WebRTC hook mapping opaque frame/connection IDs
  back to the exact `WebContents`. It reports `ObservedPeerConnection` or
  `Unknown`, **never Clear**, and does not freeze a view.
- A separate renderer/browser frozen-registry witness for disposable pages,
  checked against the exact frame/document set, with a dedicated control receiver
  on every frame. It reports a historical result and releases its freeze before
  completion; it does not grant an ongoing admission lease.
- A sandbox-preserving, workspace-local source build and disposable native
  harness. Neither the patch nor its diagnostic API is linked into Ferric.

On 2026-10-01 the full patched engine built and installed locally. Three
fresh-process native runs each passed all 40 baseline checks, including exact
14-record canonical copies, 15-record nonced-scope refusal with an empty child,
owner-loss/refusal fixtures and correct-page observation of unnegotiated and
connected data-only peers. Control pages remained Unknown. The runtime reports
Qt/WebEngine 6.11.2, Chromium 140.0.7339.225 and security patch 151.0.7922.71;
the source lock records hashes of the actual local binaries and test logs.

The follow-on HTTPS fixture also passed three times (26 checks per run): 19
canonical records created by real loopback responses and a script copied
exactly, and the child preserved the tested request behavior. This includes
host/domain/path, HttpOnly, prefixes, cross-site Strict/Lax/None behavior and
same-top-site partition ancestry collisions. The original 40-check baseline
also passed three times on that extended engine. The remaining legacy/expiry/
fault cases, source-T0/ownership protocol and RTC admission are not qualified.

The v3 frozen-registry fixture passed 35 checks in three fresh-process runs under
Qt's default isolation and three more with the fixed `--site-per-process` switch.
Both cross-origin cases in each stricter run observed two actual renderer
processes. Empty/unnegotiated/closed-retained registries, freeze-handler and
microtask construction, local/cross-process children, frame changes, navigation,
visibility, opener-related refusal, owner loss, overlap and frame-budget refusal
are included. The original 40-check and HTTPS 26-check suites each passed three
times on v3. This is evidence of a working native witness, not production
`Clear`: the freeze is released, all media/capture categories are not covered,
and retained ownership/injected-script/fault qualification remains open.

This is positive **native baseline evidence**, not an implemented/qualified
production backend. The result does not prove concurrent source snapshot,
fresh-child exclusive ownership, generic cookie losslessness, race-free
WebRTC check-and-freeze or live merge. Those work packages remain open.

## Research outcome and remaining qualification

The authorized isolated build/probe baseline, scoped HTTPS fixture and scoped v3 frozen-registry witness have passed; do not restart them from scratch or treat their compilation as the current blocker. Continue the documented disposable qualification tests without a new permission checkpoint. Legacy/expiry/fault cases and snapshot/seed ownership remain open. The v3 witness restores its own freeze and cannot grant ongoing admission. WebRTC admission still needs retained ownership and complete activity coverage beyond this historical witness. Merge remains separately gated. The owner approved pursuing a maintained shipping fork on 2026-10-08, but its platform, security and capability release gates remain separate. Until native certificates and integration evidence exist, the default public backend, production capability flags and blocked downstream slice statuses remain unchanged.

2026-10-07 RTC follow-up: the pinned Chromium `WebContentsObserver` capture
callback and internal per-frame capture query were wired into a disposable
Qt-only overlay, then exercised under strict site isolation. The overlay built,
but its test-only synthetic child-RFH event was not reflected in the exact-view
query after an empty lease; a second run failed earlier during cross-origin
lease acquisition. A real child-frame `getUserMedia` attempt also aborted as
invalid state. This does not qualify the private-API route or change this
decision: Ferric production remains on the public adapter boundary, unknown RTC
activity refuses retention, and `versioning.live_retention` remains disabled.
See [VB-033](../testing/versioned-browsing/VB-033.md) and the
[source-locked probe](../../tools/versioning-engine-spike/source-lock.json).

## Shipping-backend decision checkpoint — 2026-10-07 (superseded direction)

**At this checkpoint, the decision was not to adopt or ship the patched engine yet.** Keep the public Qt
adapter for structural restore, clean off-record branches and the exact Save
Page archive path; retain `Unknown`/unavailable outcomes for strict WebRTC
absence, cookie snapshot/seed/merge and ordinary viewless downloads. This is
not a judgment that the full feature is impossible: the isolated prototype
demonstrates several underlying primitives, but it does not supply the
production owner, race, interruption and update contracts required by this
ADR. The newer WebEngine 6.140.0 RC did not repair the tested MV3 extension
activation crash; a future final runtime must be tested, not assumed to fix it
([VB-097](../testing/versioned-browsing/VB-097.md)).

A reviewable shipping proposal must bring these pieces together as one
version-pinned backend, not as independent test hooks:

1. An exact Qt Quick view/profile ownership bridge, with generation tokens,
   bounded lifetime, no public debugger endpoint and no page-script authority.
   It must turn the RTC witness into a retained admission lease that covers
   PeerConnections in every live frame plus all media/capture and script
   paths; missed events, owner loss or renderer/network-service faults yield
   `Unknown` and release the lease.
2. A network-service cookie bridge that emits the full canonical identity and
   an engine-issued complete snapshot boundary, seeds only an unpublished
   empty child, reads every result back and discards a partially seeded child.
   Live write-back is a separate protocol requiring a source revision and
   complete competing-writer control; without it `cookie_merge_apply` stays
   unavailable even if snapshot/seed passes.
3. A separately qualified ordinary-download origin or public adapter that
   produces an exact-profile null-view request and carries it through the
   installed-byte/durable-ledger path. The page-owned `download()` API and
   the exact Save Page permit do not meet that criterion.

The maintenance commitment is material. The local isolated engine workspace
currently occupies approximately 13 GiB (`du -sh
target/versioning-engine-spike`, 2026-10-07); this is a local development
footprint, not a release-size estimate. Each upstream Qt/Chromium security
update would require rebasing the bridge at the private ownership seams,
rebuilding the patched engine for supported platforms, reproducing source and
binary fingerprints, rerunning the native race/fault/compatibility suites,
and shipping a rollback path. The release team would also own timely Chromium
security updates, sandbox and distribution review, licences/notices and
support for the pinned engine when system Qt differs. A shipping approval
therefore needs a named maintainer and security-update SLA, CI/build capacity,
target-platform matrix, signed/reproducible artifact plan, independent security
review, and a per-capability acceptance report. The isolated research
authorization does not supply any of those commitments or authorize a fork
release.

## Owner direction and integration handoff — 2026-10-08

The owner has now approved pursuing the maintained pinned backend for the full
vision. The 2026-10-07 refusal to **automatically adopt** the diagnostic build
still applies: its private `ForProbe` methods and scattered overlays are not a
shipping API, and no capability or backlog status changes from the approval.
The [development verifier and safe-mode runner](../../tools/versioning-engine-backend/README.md)
pin the currently observed Linux x86_64 research artifacts, check the runtime
library/process/QML/resource/locale tuple, and refuse a mixed engine instead of
falling back to system Qt. A bounded offscreen safe-mode smoke launch survived
18 seconds on that tuple. A live process-map check found the pinned Core,
Quick and QtWebEngine QML plugin loaded from the same local install. The
existing exact-view host QML fixture passed 18/18 tests on the candidate,
with three Chromium rejected-widget-message diagnostics. A disposable public-Qt
control also passed 18/18 and emitted five of the same message class, so they
are not unique to the patch but still require release investigation. These are
loader and scoped host-compatibility evidence, not
native journey or absence/snapshot/download qualification.
The disposable renderer on this host reported `NoNewPrivs: 1`, `Seccomp: 2`
and one seccomp filter; the intermediate zygote reported `Seccomp: 0` and its
command line included Qt's `--no-zygote-sandbox`. This is not a complete
sandbox audit or a reason to relax the release sandbox gate.

The next implementation milestone is a minimal production patch and Ferric
owner/generation bridge, followed by separate RTC, cookie and ordinary-download
native certificates and the final release campaign. Linux x86_64 is only the
initial development tuple. A named maintainer, security-update SLA, target
release platforms, CI/build capacity, signed artifacts, independent security
review and rollback remain release decisions, not assumptions supplied by
this approval. The public-Qt path remains the default and all strict
capabilities remain unavailable until their exact certificates pass.
