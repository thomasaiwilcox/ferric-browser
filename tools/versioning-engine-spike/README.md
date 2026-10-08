# Isolated native capability probes

Research-only Qt WebEngine 6.11.2 patch and harness, authorized by the project
owner on 2026-09-30. This is **not a production backend**, does not modify the
Ferric build, and does not enable any capability. Actual outcomes and next steps
are in [the evidence record](../../docs/testing/versioned-browsing/engine-capability-spike.md).

On 2026-10-01 the full engine built and installed locally, and three fresh-process
native runs each passed all 40 baseline checks. Runtime Qt/WebEngine was 6.11.2,
Chromium 140.0.7339.225, security patch 151.0.7922.71. Source, binary and passing-log
digests are recorded in `source-lock.json`. This does not qualify production
snapshot/seed, live merge or race-safe RTC admission.
That passing v1 source/binary/log set is preserved in the local archive named in
`source-lock.json`. The archived v2 patch also passes a real loopback HTTPS copy
fixture: three runs, 26 checks each, with exact 19-record read-back and matching
source/child request behavior. Broader qualification remains incomplete.

The current v3 source adds a frozen-renderer registry witness. Its full engine
build and native tests passed: three 35-check runs in the default configuration
and three 35-check runs with strict site isolation and actual cross-process frames.
The original 40-check and HTTPS 26-check regressions also passed three times each
on this engine. Separate source/binary/log digests bind these results to v3.
It pre-binds a dedicated control receiver on the agent-group default task runner,
checks exact frame/document identities, requests freeze, then reads native
frozen-state and per-document peer registry booleans. It restores its own freeze
before completion. `EmptyRegistriesAfterFreeze` is a historical diagnostic, not
a retained lease or production `Clear` result. No raw RTC metadata is exposed.

The current v4 patch adds a bounded, off-the-record retained-freeze lease
experiment. Its first native run found that Qt could execute an app-requested
script while Frozen and the lease was held. The central adapter now invalidates
and thaws before dispatching a newly requested script. The revised harness
passes 50 checks in both default and strict-isolation configurations, including
main-page and child-frame injection; one 40-check baseline and one 26-check
HTTPS regression also pass. This is not a production absence certificate:
queued/in-flight scripts, other media/capture categories and real Ferric
ownership integration remain open. Production feature flags are still disabled.

The v5 research extension tracks explicit Qt application-script dispatches,
including fire-and-forget child-frame requests, until Chromium returns a result.
Lease acquisition refuses while a request is pending, and a request racing
admission cannot leave a lease held. The native RTC harness passes 59 checks in
both default and strict-isolation modes; the 40/26-check regressions still pass.
This does not cover all renderer-side scripts, missing engine acknowledgements,
media/capture activity or production Qt Quick ownership. It does not enable a
production capability.

An additional source-pinned RTC media-activity overlay and native fixture were
built and exercised in one fresh process per configuration on 2026-10-04. The overlay holds a bounded per-WebContents
pending-media count from request admission through Qt permission handling and
until the active stream counter has been incremented or the request fails. It
starts before both the queued permission path and Qt's preselected-desktop
fast path, invalidates an in-progress/held absence lease when another media
request arrives, and refuses the experimental lease unless all existing Qt
audio/video/mirroring/desktop counters and the pending count are exactly zero.
The authored fake-audio fixture holds permission pending, checks that pending
and active media each refuse a lease, and then checks that stopping the stream
releases the interlock. A separate off-the-record fixture denies a pending
audio prompt and waits for a fresh empty-registry lease, with a stop handler
for any unexpected success. A selected-desktop request is source-wired through
the same token, and a separate test-only native seam exercises its invalid-
device fast path while a lease is held. That case checks synchronous
invalidation and refusal without starting a desktop stream. A separate
`screen:-3:0` fixture now calls the real page-side legacy desktop `getUserMedia`
path against Chromium's fake-source identifier. An additional disposable
engine overlay, `qt-6.11.2-rtc-fake-desktop-probe.patch`, equips only that fake
ID with a bounded synthetic frame generator, so the fixture can observe actual
capture-device start, a delivered video frame, lease refusal while active, and
cleanup after stopping the track without reading any OS screen. A separate
native UI seam still calls
`MediaStreamUI::OnStarted` to test the pending-to-active handoff directly. The
counter fixture forces an isolated active count to
`INT_MAX`, sends an unmatched stop at zero, and forces the pending count past
its bound, requiring all three cases to poison the absence result before
restoring the empty fixture state. The native and strict-site-isolation runs
passed, including delayed permission, fake-audio start/stop, denial, synthetic
desktop capture start, a nonzero frame read directly from the media track,
active-capture refusal, stop cleanup, and fail-closed counter boundaries. An
offscreen compositor callback did not provide a dependable frame-arrival
signal, so the fixture now uses `MediaStreamTrackProcessor` on the track itself.
These are one-run scoped research results, not final qualification; other
media/capture categories and production ownership remain open, and production
flags stay disabled.

The RTC admission and synthetic-desktop overlays were built and exercised in
the isolated engine. Their patch hashes, fixture hashes and runtime limits are
recorded in `source-lock.json`.

The current HTTPS edge fixture adds real response-setter cases on separate
disposable profiles: source-side far-future expiry clamping, expired/invalid
attribute rejection, and exact copy of the two accepted canonical records.
It passes 33 checks; the 59/59 RTC and 40-check baseline regressions pass.
Legacy provenance, opaque keys, older-expiry import, network-service faults and
snapshot/seed ownership remain open.

The v6 research method substitutes one unsupported candidate **in memory after**
reading a safe 14-record synthetic source. Both `kUnknown` provenance and an
800-day expiry refuse the entire copy before any child write; a different fresh
child still copies the real source exactly. The native baseline passes 46 checks,
with 33-check HTTPS and 59/59-check RTC regressions. This is a preflight-order
test, not a real legacy-profile import or older-cookie migration certificate.

The v7 fixture writes an actual opaque-partition cookie to a disposable native
source store alongside the 14 safe records. Its 15-record source snapshot
refuses wholesale before writing a child. An independent existing-source copy
to a second fresh child also refuses, so the result is not merely a fabricated
candidate. The rebuilt native probe passes 49 checks, with 33 HTTPS and 59/59
RTC regression checks passing. This qualifies the scoped refusal path only;
opaque-key import, real legacy migration, service faults and production
snapshot/seed ownership remain open. Digests are in `source-lock.json`.

The v8 fixture exercises a rejected destination setter after one accepted
child write. It uses an insecure URL for a Secure cookie to obtain an actual
native rejection, reports the child count as unknown (`-1`) once mutation has
begun, then diagnostically confirms that the disposable child contains one
record and the source remains complete. Its 53-check baseline, 33-check HTTPS
and 59/59 RTC regressions pass. The failed child is discarded, never exposed
as a branch. This does not qualify network-service crash/lost-callback/timeout
handling or atomic multi-cookie writes. Exact digests are in the source lock.

For the next real legacy/older-expiry source fixture, do not inject
`CookieSourceType::kUnknown` through the running Qt setter: this Chromium pin
uses `CHECK_IS_TEST()` on that path in non-test Linux builds. The ordinary
canonical setter also cannot represent an old 800-day expiry that is accepted
only from storage. The separate `probe-legacy` harness creates two synthetic
persistent Qt profiles, each with a target and safe sibling, and first proves
that both disk-loaded records copy exactly to a fresh child. It then lets each
control process exit, makes one fixed-field update to each closed disposable
`Cookies` database, and reopens it through the pinned engine's ordinary
`FromStorage` path. It issues no SQLite `SELECT` or row export, does not copy a
profile, touch a user profile, or route cookie data through Ferric. One case
tests stored unknown-source provenance; the other tests an unexpired 600-day
creation-to-expiry span. Three fresh runs each passed 16 native checks: the
two-record control copies exactly, and each changed two-record source refuses
the *entire* copy before a child write. Digests and retained disposable paths
are in `source-lock.json`. This qualifies only the scoped disk-loaded refusal,
not legacy import, lossless older-expiry copying, or production ownership.
Direct backing-store mutation is research-fixture setup only, never a
production adapter strategy.

An additional `qt-6.11.2-seed-if-empty-probe.patch` overlays the preserved v8
engine patch. It adds a research-only Mojo call and a CookieMonster-sequence
empty-jar check before full-canonical insertion, plus a separate `probe-seed`
mode that uses fresh disposable persistent children. Its zero-fuzz reverse
patch check and incremental build pass. Three fresh `probe-seed` runs passed
40 checks each with zero failures; the 53/33/59/59-check cookie, HTTPS and
RTC regressions also passed. The method is not a writer lease, cannot qualify
production ownership, and must never be called on a live profile.
The mode reopens each child in a separate process after exact in-process
read-back. A native read then finds both canonical records and the edge record
still triggers refusal of an ordinary copy. Qt's ordinary `cookieAdded` name
signals were not observed on restart, even after registering the listener
before page creation; this is a separate diagnostic, not the native persistence
criterion. The restart witness is not a second exact canonical-member
comparison across processes.
The seed callback and in-process read-back precede SQLite's batched disk
commit. The restart stage witnesses normal-close persistence only, not a
crash-durable publication or a production flush/failure contract.
Before mutating either source fixture, the mode also prepares a separate
disposable child containing one ordinary marker cookie. It requires the
seed-if-empty call to refuse that child, then copies the child's full list to
an off-the-record control and requires exactly the one marker to remain. This
negative fixture passed in all three runs; the original marker remained the
child's only record. This does not exclude a concurrent writer after the check.

`qt-6.11.2-t0-race-probe.patch` is a separate overlay on that pinned seed
build. A synthetic two-cookie source triggers one native setter *after* its
complete-list callback but *before* seeding. Three fresh `run-t0.sh probe`
runs passed seven checks each: the source then held three canonical records,
while the child and an independent child copy held exactly the captured two.
The 40-check seed, 53-check cookie, 33-check HTTPS and 59/59-check RTC
regressions passed on the extended build. The T0 overlay is source-pinned,
disposable research only; it does not lease the source owner, exclude other
writers or certify a production snapshot. Its patch and binary digests are
recorded separately in `source-lock.json`, leaving the earlier seed evidence
pinned to its original overlay hash.

`qt-6.11.2-unpublished-child-probe.patch` is another separate overlay after
the T0 patch. Qt's ordinary `cookieStore()` did not bind a CookieManager for a
profile with no page, so a page-free seed initially returned `Unavailable`.
The research-only profile hook now initializes the default storage partition
and browser-process cookie manager without a `QWebEnginePage`. Three fresh
`probe-unpublished` runs passed 46 checks each: two disk-loaded edge cases
seeded exactly before child page creation, retained their native two-record
state after page publication, and survived normal-close restart. The
40/53/33/59/59 regression fixtures passed. This does not certify an exclusive
writer lease, durable publication, or production backend. Patch, runner,
binary and fixture digests are in `source-lock.json`.

## Reproduce on the current Arch Linux host

Prerequisites: the installed Qt 6.11.2 development modules, Clang, CMake, Ninja,
Node.js, Python, uv, bison, flex and system development libraries described by
[Qt's build requirements](https://doc.qt.io/qt-6/qtwebengine-platform-notes.html).
Allow substantial disk and build time for Chromium. Three compile jobs are the
default for this two-core/four-thread, 16-GiB host, using Qt's default jumbo batch
size of eight; never infer a successful WebEngine build
from CMake's exit status alone.
The build runs at reduced scheduling priority to leave room for interactive work.
Reduce `FERRIC_SPIKE_JOBS` and reconfigure if memory pressure appears.

```sh
bash tools/versioning-engine-spike/run.sh prepare
bash tools/versioning-engine-spike/run.sh configure
bash tools/versioning-engine-spike/run.sh build
# If the safety ceiling is reached (exit 124), repeat build; Ninja resumes incrementally.
bash tools/versioning-engine-spike/run.sh install-probe
bash tools/versioning-engine-spike/run.sh probe
# After building the existing-cookie extension, run the response/request fixture:
bash tools/versioning-engine-spike/run.sh probe-https
bash tools/versioning-engine-spike/run.sh probe-rtc
# Separate, stronger-isolation tuple; requires actual multi-process frame coverage:
bash tools/versioning-engine-spike/run.sh probe-rtc-oop
# Exercise the bounded empty/data-only witness through the QML WebEngineView owner:
bash tools/versioning-engine-spike/run.sh probe-rtc-quick
# Repeat it with a cross-origin child in a strict, separate-renderer process:
bash tools/versioning-engine-spike/run.sh probe-rtc-quick-oop
bash tools/versioning-engine-spike/run.sh probe-legacy
# Research-only seed overlay, using fresh synthetic persistent profiles:
bash tools/versioning-engine-spike/run.sh probe-seed
# Separate post-snapshot synthetic write, after the seed overlay:
bash tools/versioning-engine-spike/run-t0.sh apply
bash tools/versioning-engine-spike/run-t0.sh build
bash tools/versioning-engine-spike/run-t0.sh install
bash tools/versioning-engine-spike/run-t0.sh probe
# Separate page-free child initialization and seed fixture after T0:
bash tools/versioning-engine-spike/run-unpublished.sh apply
bash tools/versioning-engine-spike/run-unpublished.sh build
bash tools/versioning-engine-spike/run-unpublished.sh install
bash tools/versioning-engine-spike/run-unpublished.sh probe
```

`baseline` runs those stages consecutively and stops on the first error. Do not
run multiple builds against the same output directory simultaneously. Passing
this baseline still does not qualify a production capability.

`FERRIC_SPIKE_BUILD_SECONDS` controls the foreground build safety ceiling (default
43200 seconds, not an instruction to stop work at a short checkpoint);
`FERRIC_SPIKE_JOBS` accepts 1–4 (reconfigure when changing it, because Qt bakes the
inner Ninja limit into the generated build). No scheduler or detached worker is created.
Preparation verifies the release archive and applies the checked-in patch.
Missing Python dependencies and gperf are installed/unpacked **only** under
ignored `target/versioning-engine-spike`. The gperf package was independently
signature-verified during initial preparation; subsequent runs verify its pinned
SHA-256. There is no system package installation or global environment change.
The source pin, package digest and dependency versions are in `source-lock.json`.

The local installation is `target/versioning-engine-spike/install`; the runner
explicitly selects its WebEngine library, subprocess and resources. Each probe
run creates a disposable XDG directory and uses off-the-record profiles. The
baseline uses `about:blank` plus synthetic in-renderer fixtures; `probe-https`
adds an owned loopback TLS server and four fixed `.localhost` names.
No cookie database, debugger
endpoint, real profile, real credential or external account is accessed. Sandbox
overrides are refused. Probe runtime directories are retained, not silently
deleted. Do not distribute this experimental engine: release SBOM, licensing,
security updates and deployment qualification are not completed.
The harness also refuses unexpected command-line arguments; it accepts no engine
flag passthrough. RTC modes select only the fixed
`--use-fake-device-for-media-stream` fixture switch; `probe-rtc-oop` adds the
single fixed `--site-per-process` switch to strengthen Qt's default process
isolation. Only the RTC harness accepts those exact combinations; the other
harnesses still refuse all flags.
Inherited flags are cleared, and no system or Ferric setting changes.
The harness puts the patched module's header root before system Qt's roots;
otherwise Qt's forwarding headers can select the unpatched system headers even
when CMake selected the local library. The offscreen runner clears an inherited
desktop platform theme and uses Fusion only for its own process, so a GTK theme
does not require a live display. It changes no desktop settings.

## What the probe actually does

The compiled patch adds diagnostic-only C++ methods (not QML properties or
invokables). The cookie method requires two unused disposable stores and refuses
nonempty stores. It writes 14 synthetic canonical records into the source,
captures through the real `CookieManager::GetAllCookies` completion callback,
sets canonical records into the child with individual outcomes, and compares
every canonical data member on source and child read-back. Covered fixture
dimensions include:

- Host/domain scope, path, expiry/session, Secure/HttpOnly, all four SameSite
  policies, low/medium/high priority, source scheme/port/type and timestamps.
- Four otherwise-colliding records: unpartitioned, top-site A/same-site ancestry,
  top-site A/cross-site ancestry, top-site B/cross-site ancestry.
- A separate 15-record fixture with a nonced partition key: refuse the entire
  clone before child writes, and independently read back an empty child.
- Null/same-store destinations, source/destination cookie-filter refusals,
  overlapping requests, populated-source and
  populated-destination refusals, and destruction of either owner. A subsequent
  successful clone checks that destination refusal left the source empty and
  released its operation guard. A 20-second deadline bounds the engine operation.

The RTC method subscribes to Chromium's browser-side PeerConnection observer,
retains at most 4096 opaque frame/connection pairs, and maps positive observations
to the exact WebContents. Zero observations, overflow or unavailable state are
`Unknown`. The harness creates two local data-only peers without STUN/TURN and
checks the positive page and a distinct negative-control page. A separate fresh
fixture checks construction without offers or ICE candidates; it is destroyed
before the connected-pair test so a retained handler cannot satisfy both checks.
Each RTC case includes both same-profile and different-profile control pages.
Its local signalling
waits for ICE gathering before handing descriptions to the other peer; signalling
failure is reported without disclosing descriptions/candidates. It never freezes
the page. It retains no URLs, configurations, SDP, ICE data, stats or messages.

## HTTPS follow-on (26 checks passed in three runs)

`cookie_request_probe.cpp` serves only an ephemeral loopback TLS endpoint using
[QSslServer](https://doc.qt.io/qt-6/qsslserver.html). The runner generates a
one-day disposable certificate/key under that run's private directory. The page
accepts only the exact pinned certificate, expected authority error, fixed
`.localhost` names and owned port. No system trust store, host mapping, debugger
or browser security flag is changed. The fixture bounds its connections, request
buffers and retained boolean observations; it does not log cookie headers.

The research-only `ferricCookieCopyExistingProbe` reuses the native guarded copy
operation without inserting the original fixed fixture. The new fixture creates
13 response-set ordinary cookies, one script cookie and five same-name partition
variants (unpartitioned, first-party/same-site ancestry, that same top-site with
cross-site ancestry, top-site A and top-site B). It checks source
request behavior, exact native 19-to-19 read-back, then the child's requests.
Host/domain/path scope, HttpOnly visibility, cookie prefixes and a cross-site
navigation's Strict/Lax/None behavior are included. Chromium's expected -202
certificate diagnostics remain in the logs before the exact pinned test decision;
they are not cookie-test failures. Expiry races, rejected writes, persistent store
loading and the other qualification work packages remain separate.

## Frozen-registry follow-on (35 checks, three runs per configuration)

`rtc_registry_probe.cpp` uses the shared owned HTTPS fixture to exercise empty
registries, unnegotiated and closed-but-retained connections, connections created
by freeze callbacks and their microtasks, and same-origin/cross-origin children.
The OOP mode requires two observed renderer processes for its two cross-origin
cases. Frame creation/removal during freeze, navigation, visibility changes,
opener-related content, overlapping probes, owner destruction and a 129-frame
scope all fail closed. A pre-existing freeze is not thawed by a refused request.
Successful probes restore Active and wait for a real renderer task before reuse.

The renderer observer is created for every frame, independently of Qt's existing
main-frame-only page interface. Warmup binds its dedicated control receiver
before freezing. Every reply is checked against the captured document identity
and complete frame set, and an empty result requires actual native frozen state
from every recorded frame. The earlier main-frame-only attempt correctly returned
Unknown for missing child receivers; its failed log is retained in the evidence record.

The RTC media-activity overlay and synthetic `screen:-3:0` fake-desktop fixture
are built into the isolated Qt engine and passed one fresh native and one fresh
strict-site-isolation run. The fixtures cover held permission, fake audio,
denial, preselected-desktop handoff, counter bounds, actual synthetic frame
delivery, refusal while capture is live and stop cleanup. The frame check reads
the captured track through `MediaStreamTrackProcessor`, not the offscreen
compositor. The separate `qt-6.11.2-rtc-quick-bridge-probe.patch` adds a
test-only wrapper on the actual QML `WebEngineView` owner, while
`rtc_quick_bridge_probe.qml` exercises empty admission, script-dispatch
invalidation, overlapping-acquire refusal and data-only peer refusal. Its patch
in its last-exercised form passed a zero-fuzz source check, built and passed one
fresh off-the-record QML run: the exact view acquired an empty lease, refused
overlap, revoked before script dispatch, then observed and refused the data-only
peer. Runtime owner generation and adapter-replacement ordering are included.
The current patch and fixture have since been extended and are not yet checked
or run; see the current and last-exercised digests in `source-lock.json`. The
previous results qualify only the scoped pinned paths; they do not qualify all
activity categories or production ownership. Production stays disabled. Run
`probe-rtc-quick` to repeat the Qt Quick tuple. Other renderer activity, native
fault/loss cases, production Rust ownership and release review remain open; the
QML bridge is not a shipping Qt API.

The RTC registry fixture was subsequently extended with a camera-only
`getUserMedia({video:true})` route using Chromium's fake media device. It now
holds video permission pending, requires pending/active lease refusal, checks
a synthetic track frame, and awaits stop cleanup. This newest fixture has not
been built or run; the recorded default and strict-isolation results refer to
the prior fixture digest in `source-lock.json`. The extension never selects an
OS camera and does not change any capability status.

The Qt Quick exact-owner fixture was also extended to run a fake camera stream
through the same `WebEngineView` wrapper: it grants the fake video permission,
requires dispatch invalidation, observes a synthetic frame, tests refusal while
active, then stops the track and waits for the empty lease before its peer test.
That latest QML fixture has not been built or run; `source-lock.json` preserves
the earlier fixture digest associated with the previous pass. It never selects
an OS camera and is not a production capability result.

The current exact-owner fixture additionally checks visible/Frozen lifecycle
transitions, a synthetic audio-only microphone request with permission held
pending, active-track refusal, cleanup, and a second acquisition racing an
immediate transition to visible/Active. Source review found that the isolated
patch's lifecycle handler omitted an acquisition still pending at that point;
the patch now cancels pending as well as installed leases. These latest patch
and fixture changes are not patch-checked, built or run. Prior hashes and the
one-run result remain recorded separately in `source-lock.json`; no production
capability claim changes.

The latest fixture also replaces the leased exact view's profile with a second
off-the-record profile, requires the old adapter lease to be invalidated, waits
for the replacement page reload, and then obtains a new empty observation.
This update is authored but not executed; earlier native results remain tied to
the prior patch/fixture tuple.

The current exact-owner fixture also adds combined synthetic audio/video
capture: it holds the combined permission pending, checks refusal while pending
and active, reads both live tracks and a synthetic video frame, then checks
fresh admission after stopping both tracks. This extension is authored but not
built or run. The native RTC fixture separately adds a Linux pidfd-guarded
renderer-loss case; it verifies the renderer as a child of the disposable
probe before signalling and expects lease invalidation plus an `Unknown`
follow-up. That current fixture is also unrun; previous native results remain
bound to the `last_exercised_fixture_sha256` value in `source-lock.json`.

The native RTC fixture now also mirrors the combined synthetic audio/video
case from the exact-view QML probe. It holds the combined media permission,
checks pending refusal, grants only fake devices, reads both live tracks and a
synthetic video frame, refuses active capture, and waits for empty admission
after both tracks stop. A separate fresh off-the-record page denies the
combined permission and verifies pending-activity cleanup. The latest native
fixture hash remains unbuilt/unrun; previous run results still refer only to
`last_exercised_fixture_sha256`.

The exact Qt Quick owner fixture now closes its data-only peer, reacquires an
empty lease on that same view, and checks renderer-loss invalidation. Its C++
runner reads only the QML view's renderer PID, requires exact child identity
and a start-time recheck, then uses Linux pidfd signalling. The QML fixture
requires the render-termination signal and exactly one owner-bridge
invalidation. This latest Quick fixture pair is unbuilt/unrun; earlier results
remain bound to its previous source hashes.

Static review found that the native admission entry point requires hidden
`Active` state before it initiates its own freeze, while later Quick fixture
legs re-entered from hidden `Frozen`. The disposable Quick bridge now rejects
visible/non-admissible owners and explicitly thaws a hidden `Frozen` view before
calling native admission; lease retention still requires the exact view to be
hidden and `Frozen` at completion. The QML fixture now also explicitly checks
that an immediate acquire against the visible owner is refused. These
methods now reject foreign-thread calls before reading private Quick-view
state. The C++/QML runner now invokes acquire/check/release from a foreign
thread while the exact lease is held, then requires the lease to remain valid.
These corrections and assertions are authored but have not been patch-checked
or run, so they add no qualified result.

## Deliberate limits

- `ExactReadback` is a synthetic diagnostic result, **not** a snapshot/seed
  capability certificate. An empty store is not proof of a fresh, never-navigated,
  exclusively owned destination. The harness creates the profiles and loads
  only `about:blank` to initialize the network context.
- Cookie vectors stay in native memory and are promptly discarded, but standard
  C++ string destruction is not certified secure zeroization. They never appear
  in QML, Ferric's JSON IPC or harness output; only fixed labels, enum codes and
  counts are printed. Chromium's existing internal Mojo calls do carry canonical
  records between engine components; this is not a claim that cookies never
  cross an internal engine-process boundary.
- The post-enumeration count bound is adequate for this fixed synthetic fixture,
  not a production pre-allocation limit. A production protocol must enforce byte
  and count limits before allocating/sending arbitrary-profile records.
- The HTTPS fixture now exercises the specific request/CHIPS cases above. It
  does not qualify disk-backed sources, concurrent writers, restart, expiry
  races, network-service loss or merge.
- A mutation failure, read-back mismatch or timeout can leave partial writes.
  Ending the operation and ignoring late callbacks does not retract a write
  already sent to Chromium. Discard the affected disposable profiles rather than
  retrying them; the overlap guard is not a cancellation or rollback protocol.
- Nonced/opaque keys are not silently downgraded. Unsupported source attributes
  must refuse the entire requested scope in the future backend.
- RTC positive observation does not prove connection liveness, comprehensive
  frame coverage or absence. A missing notification never becomes `Clear`.
  The separate frozen-registry witness is diagnostic only: it releases the freeze
  before completion. Retained admission, all media/capture activity, application
  script races, crash/timeout fault injection and the Qt Quick owner bridge are
  **not qualified** by these fixtures.
- The harness uses native `QWebEnginePage`, not Ferric's Qt Quick views. A
  production Qt Quick/Rust ownership bridge remains separate work after the
  underlying native guarantee has been qualified.

These native baselines now pass. Continue with the remaining cookie fault/legacy/
expiry and source/child ownership cases and retained RTC admission described in
the evidence record; preserve the baselines as regression checks.
Keep `qt-public-default` and all production gates
unchanged until the corresponding complete native tests and integration pass.

The legacy-cookie owner-loss follow-up exercises both destination teardown
(including source reuse) and source teardown while the native reply is pending.
Three fresh disposable campaigns passed 32 checks each. This is scoped
QObject/profile callback-retirement evidence only; it does not establish the
production source-lock capability. A possibly mutated child is discarded
rather than reused.

The latest separately recorded native fault campaign disconnects the bound
CookieManager Mojo receiver during a pending `GetAllCookies` reply. Three
source-loss and three destination-loss cases completed exactly once as
`TimedOut`; a destination with unknown post-timeout contents was discarded.
This is not a network-service process crash/restart test and does not establish
writer exclusion, durable publication or production seed support. A 2026-10-06
source-only audit found that CookieManager, RestrictedCookieManager and HTTP
response setters converge on the CookieStore/CookieMonster add path, but
deletion/cleanup methods remain distinct. Any true engine mutation lease must
cover every store mutation or rely on a proved page-free, unpublished child.
The existing child prototype narrows the surface but does not prove the latter
invariant. See the [capability evidence record](../../docs/testing/versioned-browsing/engine-capability-spike.md)
and VB-075; production capability flags stay disabled.

The exact Qt Quick RTC fixture now creates and closes its synthetic data-only
peers through `WebEngineScript.ApplicationWorld`, matching Ferric's browser-
owned script world. Three fresh off-the-record processes passed on 2026-10-06.
On 2026-10-07, three further runs passed with a synthetic fake-microphone
permission/stream lifecycle also dispatched in ApplicationWorld: pending and
active capture refused the diagnostic lease and stopping the track allowed a
fresh empty observation. This narrows the world-coverage question for the
pinned prototype; it does not establish complete RTC/media absence, production
Rust ownership, or shipping approval. See
[`source-lock.json`](source-lock.json) and the VB-033 evidence record.

On 2026-10-07, the exact QML owner also passed three strict
`probe-rtc-quick-oop` runs after enumerating `view.mainFrame.children` and
dispatching peer creation directly to the cross-origin frame in
`ApplicationWorld`. The exact-view registry reported at least two frames and
two renderer processes, and reloading the view cleared the child registration;
one default-configuration run passed as well. This provides a concrete
cross-frame implementation path through Qt's `WebEngineFrame` API, but not an
exhaustive RTC-absence guarantee or production capability. Keep VB-033 blocked
and live retention disabled until the full ownership boundary is qualified.

The pinned Chromium capture-state route was then implemented only in a separate
test overlay: it adds a whole-`WebContents` per-frame capture query and a
`WebContentsObserver` invalidation callback. The normal strict OOPIF control
still passed after isolating this experiment. The dedicated
`probe-rtc-quick-capture-oop` run did not qualify the route: one fresh run
reached the frozen exact-view lease but the synthetic RFH counter injection
returned false, with no frame capture visible; another ended during the
cross-origin empty-lease callback. An earlier real child-frame `getUserMedia`
attempt returned `AbortError: Invalid state`. These are bounded failed probe
results, not evidence of real capture or a usable Clear certificate. The
overlay uses Chromium internals and remains outside Ferric production; keep
unknown activity refusing and `versioning.live_retention` disabled. Exact
patch, runtime and failure-log hashes are in `source-lock.json` and VB-033.

Follow-up on 2026-10-07 retried real child-frame `getUserMedia` with the fake
camera. Five strict-site-isolation attempts failed with `AbortError: Invalid
state`; the child was complete, visible, secure and had effective camera
Permissions Policy, both page RFHs were active, and the Chromium media
dispatcher entry was reached. No QML permission callback or live track was
observed. The same failure reproduced without strict site isolation. The
exact denial branch remains unknown; do not interpret the Qt source's
empty-device-set mapping as proof that it ran. `queryPermission()` cannot
pre-grant the nonpersistent video-capture permission. A strict-site-isolation
same-origin child control failed identically, so cross-origin policy and
OOPIF separation alone do not explain the symptom. Production remains
disabled. See `child_frame_real_media_permission_followup` in
`source-lock.json` for inputs and log digests.

Subsequent controlled strict-site-isolation runs (three fresh off-the-record
processes) did observe a live fake-camera track in the cross-origin child,
activity in the exact-view capture query, refusal from the inverse no-capture
query, then track-stop cleanup and a new empty lease. The QML result retains
`childCaptureTrack=true` and `childPermissionCallback=false`. Authorization is
still unresolved: the test-only Qt grant mode's permission trace did not
appear, while an exact-origin deny-mode control returned the same
`AbortError: Invalid state` as the normal run and also emitted no permission
trace. These results demonstrate a diagnostic capture/observer path, not a
known Qt grant path or production capability. Keep VB-033 blocked, unknown
activity refusing and live retention disabled; see the detailed entries in
[`source-lock.json`](source-lock.json).

Correction, 2026-10-07: direct Qt stderr traces later established that both the
test-only exact-origin grant and deny branches were executed. More importantly,
the ordinary request path reached QML with permission type `2`; Qt 6.11.2's
generated metadata marks `WebEnginePermission.PermissionType` as scoped, while
the production handler had used the nonexistent flattened
`WebEnginePermission.MediaVideoCapture` member. The production controller now
uses `WebEnginePermission.PermissionType.<name>` and its mapping tests pass.
Three fresh strict-site-isolation runs with grant/deny overrides absent now
pass: the normal QML callback grants the fake child camera, the exact-view
capture query observes the live track and refuses an empty lease, and stopping
the track clears capture before fresh empty admission. This supersedes the
earlier unresolved-authority result. It remains disposable fake-device evidence;
private Chromium observation, complete RTC/media absence, production Rust
ownership and release qualification remain open, so VB-033 stays blocked and
production live retention stays disabled. See the `permission_enum_fix` entry
in [`source-lock.json`](source-lock.json) and the final section of VB-033.

2026-10-07 cookie snapshot-boundary follow-up: the disposable T0 fixture now
performs an acknowledged synthetic write before requesting `GetAllCookies`,
then retains the engine-injected write after the complete-list callback but
before seeding. Three fresh runs passed eight checks each: the child exactly
contains the two original records plus the pre-cut record; the source contains
one further post-cut record; an independent child copy remains at the captured
three-record snapshot. The probe logs only fixed labels/counts. This narrows
the native ordering evidence but does not prove simultaneous multi-route writer
exclusion, source-generation ownership, exclusive child writers, or durable
publication. Production cookie flags remain disabled. See
`native_t0_pre_and_post_cut_followup` in [`source-lock.json`](source-lock.json).

2026-10-07 public-writer-during-copy-orchestration follow-up: the fixture starts
the multi-step copy-existing probe, then dispatches a public `setCookie` before
the Qt event loop is pumped again. The probe performs source/destination
preflight reads before the actual complete-list request; in three fresh
eleven-check runs the synthetic write completed during that orchestration and
was included in the exact five-record cut. This is writer-before-cut evidence,
not a direct setter-vs-snapshot-request race. The pinned source audit below
supports a sequence-point snapshot on the tested internal route, but does not
prove complete writer exclusion, source-owner lease, child-writer isolation,
crash durability or production approval. Production cookie flags remain
disabled. Run and binary digests are in
[`source-lock.json`](source-lock.json) under
`native_t0_copy_operation_writer_followup`.

The pinned-source audit also follows Qt `CookieManager::GetAllCookies` through
Chromium `CookieMonster::GetAllCookiesAsync` to the thread-checked complete-list
copy: ordinary and partitioned maps are copied into a standalone list before
the callback. Set/delete paths use the same owning sequence. This makes a
sequence-point snapshot technically plausible for the tested internal bridge,
but is source-version-specific and does not provide a public Qt API, complete
writer inventory, Ferric owner-generation binding or durability. File hashes
are in the same `source-lock.json` record. Production remains disabled.
