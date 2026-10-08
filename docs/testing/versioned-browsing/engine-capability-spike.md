# Native engine capability spike — 2026-09-30; results 2026-10-01

## 2026-10-08 owner-approved backend direction and disposable handoff

The owner approved pursuing a maintained, version-pinned Qt/Chromium backend
for the full vision. This does **not** approve releasing the current diagnostic
overlays or enabling a strict capability. The new
[`verify-and-run.sh`](../../../tools/versioning-engine-backend/verify-and-run.sh)
pins the observed local Linux x86_64 research library/process and
QML/resource/locale hashes, refuses mismatches, and launches only a disposable
offscreen safe-mode profile. Its `check` passed; a bounded Ferric smoke launch
survived 18 seconds, and a live process map showed WebEngine Core, Quick and
the QtWebEngine QML plugin all loaded from the pinned install. The existing
exact-view host QML fixture passed 18/18 through the same launcher. Three
Chromium rejected-widget-message diagnostics appeared during that passing
fixture; a disposable public-Qt control also passed 18/18 and emitted five of
the same message class. They are not unique to the patch but remain a release
investigation item. These tests qualify only
the checked development handoff and scoped host compatibility; they are not
RTC absence, cookie snapshot/seed/merge or ordinary viewless-download
certificates. The release platform matrix and maintenance/security ownership
remain open. See [ADR-0016](../../architecture/ADR-0016-versioning-engine-capability-backend.md).
One disposable Ferric smoke run showed the pinned child process and resources;
the actual renderer reported `NoNewPrivs: 1`, `Seccomp: 2` and one seccomp
filter. The intermediate zygote reported `Seccomp: 0` and used Qt's
`--no-zygote-sandbox` switch. These point observations do not replace a
sandbox review of the full process tree and release configurations.

Status: isolated engine built and installed; native baseline passed three times
(40 checks per run). The expanded HTTPS fixture also passed three times
(26 checks per run, 19 canonical records). Broader qualification remains incomplete.
The 2026-10-05 exact Qt Quick owner/lifecycle fixture also passed one fresh
focused run, including peer registration/close semantics, same-view context
destruction and exactly-once renderer-loss invalidation. This is scoped
feasibility evidence only; production remains disabled.
The v3 frozen-registry extension passed three 35-check runs in each of two
configurations (default and strict site isolation with actual OOP frames), plus
three 40-check and three 26-check regressions. Production backend/capability flags
and downstream slice statuses: unchanged. Source/binary/log digests distinguish
the passing v1–v8 artifacts; matching version strings are not equivalence.
The v4 bounded lease prototype passed 50 native checks in each configuration;
one 40-check baseline and one 26-check HTTPS regression also passed. This does
not qualify production RTC admission or change capability flags.
The v5 pending-app-script extension passed 59 checks in each RTC configuration
and repeated the 40/26-check regressions once each; production status is unchanged.
An additional RTC media-activity interlock overlay and delayed-permission native
fixture have now been authored against the same source pin, but have **not** been
built or run. No RTC evidence count or production capability status changes from
that unverified work.
An additional HTTPS edge fixture on the same pinned engine passed 33 checks,
with 59/59 RTC and 40-check baseline regressions; package 2 remains partial.
The v6 synthetic preflight fixture passed 46 checks, plus 33 HTTPS and 59/59
RTC regressions. No production capability changed.
The v7 actual opaque-partition source fixture passed 49 checks, plus 33 HTTPS
and 59/59 RTC regressions. It proves only the scoped whole-copy refusal;
production capability remains disabled.
The v8 mid-copy destination setter rejection fixture passed 53 checks, plus
33 HTTPS and 59/59 RTC regressions. It demonstrates partial-child failure
reporting, not network-service fault atomicity; production remains disabled.
The separate seed-if-empty overlay passed three fresh disposable runs of 40
checks each: both disk-loaded edge sources seeded exactly in memory, remained
engine-readable after normal-close restart, and a nonempty child was refused.
The 53/33/59/59 baseline regressions passed. This is not production ownership,
crash durability, or a WebRTC absence certificate; flags remain disabled.
A separate post-snapshot-write overlay passed three fresh 7-check runs: after
a native callback captured two cookies, a forced source write made three, while
the seeded child still held the captured two. Its 40/53/33/59/59 regressions
passed. This is a bounded T0 ordering witness, not a source-owner lease.
The 2026-10-07 follow-up exercises both sides of that cut in one synthetic
run: an acknowledged source write before `GetAllCookies` is included in the
child snapshot, then the engine overlay inserts a second write after the
complete-list callback and before seed. Three fresh disposable runs passed
eight checks each; the source contains four records while the child remains
exactly at the three-record cut. This establishes the tested serial ordering
case, not simultaneous cross-interface writer exclusion, source-owner
generation, child writer exclusivity or durable publication; production
capability flags remain disabled. Detailed fixture/runtime/run hashes are in
`source-lock.json`.
The next 2026-10-07 follow-up dispatches a multi-step public copy-existing
probe and a public cookie setter back-to-back before pumping Qt's event loop.
Because the copy probe performs preflight reads before its actual complete-list
request, all three fresh eleven-check runs observed the setter in the exact
five-record cut. This is writer-before-cut evidence during operation
orchestration, not a direct setter-vs-`GetAllCookies` race. The source audit
below supports a sequence-point snapshot on the pinned internal route; neither
piece proves the full writer set or owner-generation lease, and production
capability remains disabled. See
`native_t0_copy_operation_writer_followup` in `source-lock.json`.
A separate page-free-child overlay passed three fresh 46-check runs. Explicit
storage-partition/cookie-manager initialization let two disk-loaded edge
sources seed exact two-record children before any child page existed; the
children retained two records after page creation and normal-close restart.
The 40/53/33/59/59 native regressions passed. This narrows the child-writer
surface but is not an exclusive writer lease or durable publication proof.
Authority: project owner requested research, authorized trying the native route,
and requested implementation-document updates so later work can continue.

## Concrete result

The isolated native route now has positive execution evidence. On 2026-10-01,
three fresh-process runs each passed all 40 baseline checks with exit status 0:
14 canonical cookie records copied exactly, including the four partition
collision variants; the 15-record nonced fixture refused the copy and read back
an empty child; and both an unnegotiated PeerConnection and a connected local
data-only pair were observed on the correct page. Same-profile and separate-profile
control pages remained Unknown. This is a working **synthetic native baseline**,
not a production cookie backend, an RTC absence certificate or a completed slice
qualification. The HTTPS follow-on now adds positive evidence for part of
package 2 below; it does not complete the remaining packages.

Package 2 remains partially complete. The passing v1 source/binary/log set was
preserved in `target/versioning-engine-spike/baseline-2026-10-01-v1.tar`
(digest in the source lock) before extending the patch. The current
extension adds a diagnostic copy of existing fixture cookies and an owned
loopback HTTPS response/request harness. Its incremental build is logged in
`build-https-extension-1.log` and `build-https-extension-2.log`; its HTTPS fixture
now passes the precise scope recorded below. The new
whole-list preflight also rejects unsupported source ports and setter expiry
normalization before child writes. This remains a bounded synthetic experiment,
not a general profile API or a fresh-child ownership certificate.
The later HTTPS edge fixture adds source-side expiry clamping and invalid
response-setter refusal; it does not finish legacy/opaque/fault or older-expiry
import cases.
The later v7 fixture demonstrates refusal for an actual opaque partition key
present in the native source store; it does not qualify importing that key.

There is a plausible bounded Qt-wrapper extension for canonical cookie
snapshot/seed. Chromium has the complete record type and a complete-list callback;
the existing public Qt wrapper discards fields and the callback. Host-only scope
is encoded in canonical domain text, so the previous claim that a dedicated flag
was inherently necessary was incorrect. A new transaction system is not known
to be necessary for a point-in-time **read**; live-profile merge is separate.
These are source findings, **not positive native capability certificates**.

The checked-in [patch, harness and runner](../../../tools/versioning-engine-spike/README.md)
are designed to exercise the candidate APIs without exposing raw cookies outside native
memory, adding a debugger, reading databases or changing Ferric's production
adapter. The baseline RTC hook exposes only `ObservedPeerConnection` and
`Unknown`. V3 adds a historical frozen-registry witness; v4 experiments with a
bounded held lease. Neither is production admission or merge.

### V4 retained-freeze lease experiment — scoped native tests passed

The current research patch can retain an empty-registry result as a bounded,
off-the-record frozen lease. It holds the exact frame/document set, page owner,
visibility and lifecycle while the caller holds a release closure; navigation,
frame changes, visibility, owner loss and the 60-second deadline invalidate and
thaw. This is a prototype of an ownership boundary, not a complete absence
certificate or Ferric integration. A first native run exposed a concrete race:
Qt's application `runJavaScript` executed an injected peer-creation script
while the page was reported Frozen and the lease was held. The current patch
therefore invalidates and thaws before dispatching app-requested scripts through
the central adapter path. Main-page and cross-origin child-frame injections now
both invalidate the lease before their script callbacks and require fresh
admission after creating a peer. The native harness passed 50 checks in each
configuration, with two actual renderer processes in the strict-isolation
cross-origin cases; baseline and HTTPS regressions passed 40 and 26 checks.
The reverse-application patch dry run matched all 13 modified files with zero
fuzz. Exact source, binary, fixture and log digests are in the source lock; the
v4 source/binary/log snapshot is preserved locally as
`target/versioning-engine-spike/baseline-2026-10-01-v4.tar`.
Already queued/in-flight scripts and other media/capture activity are still
unqualified. Production flags remain disabled.

### V5 pending application-script exclusion — scoped native tests passed

The isolated adapter now tracks explicit application `runJavaScript` requests
even without a caller callback. A lease request refuses while any such dispatch
awaits Chromium's result callback; an app script started during admission also
prevents retention, or invalidates an already retained lease before dispatch.
The native harness proved this with a delayed main-frame script, a delayed
fire-and-forget cross-origin child-frame script, and a request racing admission.
After each refusal, a fresh native registry witness observed the peer created
by that script. Both default and strict-site-isolation configurations passed
59 checks with zero failures, the latter again measuring two renderer processes
for the cross-origin frame. The baseline and HTTPS regressions passed 40 and
26 checks. Source, binary, fixture and log hashes are bound in the source lock.
The tested v5 source/binary/log snapshot is preserved locally as
`target/versioning-engine-spike/baseline-2026-10-01-v5.tar`.

This closes only the tested Qt adapter entry-point interval. A callback can be
lost on renderer failure, so the guard remains conservative rather than timing
out to `Clear`. Renderer-side script scheduling, all camera/microphone/screen
capture and other relevant activity, crashes/missing acknowledgements, Qt Quick
owner integration and release qualification remain open. No production flag is
enabled.

### V3 frozen-registry experiment — scoped native tests passed

The renderer extension reads the non-creating per-window native peer tracker on
the renderer main thread. A dedicated associated receiver is pre-bound while the
page is active, on the agent-group default task runner, so it can report actual
Page/ExecutionContext frozen state after the ordinary frame queues stop. Each
response carries only availability/frozen/registered booleans and an opaque
document identity. The browser validates the exact frame/document set (maximum
128), related-content count, page ownership and lifecycle on every round. Missing
replies, disconnects, navigation, frame changes and owner loss return Unknown;
the operation has a 20-second deadline and bounded retries. It restores only the
freeze it requested and grants no ongoing lease. It intentionally freezes owned
synthetic peer fixtures for research, not ordinary browsing pages.

`rtc_registry_probe.cpp` adds empty/unnegotiated/closed-retained peers, freeze
callbacks and microtasks, same-origin and cross-origin children, removal during
freeze, overlap, already-frozen/visible refusal and owner destruction. Cross-origin
tests require more than one observed renderer process before claiming OOP coverage.
The first engine build/install succeeded, but `native-probe-rtc-registry-1.log`
exited 1 with 26 passing and four failing assertions. The single-document witness,
freeze-handler creation and its microtask variant all passed. Child queries
returned Unknown because Qt creates `WebEnginePageRenderFrame` only for main
frames; the removal fixture consequently never reached its freeze handler.
The corrected patch creates a separate RTC observer for every frame while leaving
Qt's existing page interface main-frame-only. The incremental rebuild and local
installation succeeded (`build-rtc-registry-2.log`, `install-probe-rtc-registry-2.log`).
The corrected 30-check fixture passed three times in each configuration, then
five navigation/visibility/opener checks expanded it to 35 checks. The final
`native-probe-rtc-expanded-{1,2,3}.log` and
`native-probe-rtc-expanded-oop-{1,2,3}.log` each report 35 PASS, zero FAIL and exit 0.
Both cross-origin cases in every OOP run report exactly two frames/two processes;
the default runs report two frames/one process. These are measured, not inferred,
process scopes. `native-probe-baseline-v3-{1,2,3}.log` each passes 40 checks, and
`native-probe-https-v3-{1,2,3}.log` each passes 26 checks on the corrected engine.

The passing scope includes empty, unnegotiated and closed-retained peer registries;
peers constructed in freeze handlers and their microtasks; same-origin and
cross-origin children; and Unknown/refusal on frame creation/removal, document
navigation, visibility change, related opener content, overlap, owner destruction
and frame-budget overflow. The existing-freeze refusal does not thaw someone
else's freeze. Successful probes restore Active and await an actual renderer task.
The test reports only fixed labels, enum values, counts and configuration booleans.
It exposes no URLs, SDP, candidates, connection state details or document tokens.

This advances the feasibility finding from browser-only positive observation to
an executed renderer/browser frozen-registry witness. It still returns a historical
result after releasing its freeze, not an admission lease. It does not qualify
all media/capture activity, renderer crash/deadline faults, application-injected
script races or the production Qt Quick/Rust ownership bridge.

The pinned `ContentBrowserClientQt::ShouldEnableStrictSiteIsolation()` returns
false. The first run consequently observed one process even with cross-origin
children. The default fixture records that configuration without claiming OOP
coverage. A separate `probe-rtc-oop` mode sets only `--site-per-process`, which the
pinned `SiteIsolationPolicy::UseDedicatedProcessesForAllSites()` checks before
the embedder default. This strengthens isolation; it does not disable security
checks or the sandbox. The harness accepts only the empty flag string or that
exact switch, and the runner clears inherited flags before selecting its fixed
mode. Separate-process coverage requires an actual process count above one in
the native result. No Ferric/system default is changed. General process-isolation
background is in [Chromium's process model documentation](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/docs/process_model_and_site_isolation.md);
the pinned source, not that moving document, defines this experiment.

The earlier HTTPS source/binary
set is preserved in `baseline-2026-10-01-v2.tar` with its digest in the source lock.

Inspection of the pinned canonical-cookie Mojo traits also confirms that this
internal engine boundary carries SameSite, priority, partition site/ancestor/nonce,
source scheme/port/type and all four timestamps. Its decoder reconstructs the
canonical record from those fields. This supports the wrapper-extension route;
it does not replace the required native equality/request tests.

One concrete safety boundary is already visible in the pinned setter:
`CookieMonster::SetCanonicalCookie` invokes `CHECK_IS_TEST()` for source type
`kUnknown` on non-macOS builds. That value can legitimately describe cookies
predating the provenance field; it is not permission to rewrite it as `kOther`.
The prototype's whole-snapshot preflight rejects it before child writes. Legacy
records therefore need a separately qualified import path or an explicit refusal,
not a test-mode override or a lossy conversion.

V6 tests the **refusal path only** without constructing a real legacy profile:
after an exact safe synthetic source read, the research operation substitutes
the last in-memory candidate with either `kUnknown` provenance or an 800-day
expiry. In each case, whole-list preflight returns `UnsupportedRecord` before
any child write. The source still contains only its 14 ordinary synthetic
records, and a separate fresh child copies those actual records exactly. The
46-check native baseline passed; the 33-check HTTPS and both 59-check RTC
regressions passed on the same rebuilt engine. This does not prove how a real
old cookie database surfaces through the pinned network service, nor does it
qualify a lossless import of unsupported records. Keep such a profile refused
until an actual source/import path is qualified.
The tested v6 source/binary/log set is preserved locally as
`target/versioning-engine-spike/baseline-2026-10-01-v6.tar` with its digest in
the source lock.

V7 tests a different unsupported attribute using a real native source record.
After adding the 14 ordinary synthetic records, the pinned engine setter
accepts one `Partitioned` cookie whose partition site is opaque. The source
snapshot contains 15 records. Whole-list preflight returns
`UnsupportedRecord` before any child write, and copying that existing source
to a second fresh child produces the same refusal. This is stronger than the
v6 fabricated-candidate check for the opaque-key path: the unsupported record
was read back from the source store. It is still a disposable, off-the-record
fixture, not proof that every browser-created opaque cookie can be imported,
that a production profile has source-T0 ownership, or that mutation faults are
atomic. The rebuilt v7 engine passed 49 baseline checks, 33 HTTPS checks and
59 checks in each RTC configuration, all with zero failures. Exact source,
binary and log digests are in the source lock. Keep opaque keys refused rather
than flattening or dropping their partition identity.

V8 tests a post-preflight destination failure. After the first Secure cookie
is accepted in the disposable child, the research fixture supplies an insecure
HTTP source URL for the second Secure cookie. The native setter rejects that
write. The operation returns `MutationRejected` with destination count `-1`
(unknown), rather than falsely reporting an empty child. A separate diagnostic
copy of the failed child proves one record remained there, while a fresh copy
of the original source still reads all 14 records. The failed child is then
discarded with its disposable profile; a production branch must never expose
or reuse it. The rebuilt v8 engine passed 53 baseline checks, 33 HTTPS checks
and 59 in each RTC configuration, all with zero failures. This is a real setter
rejection induced by the fixture, but **not** a network-service crash, lost
callback, timeout, rollback, or proof of atomic multi-cookie mutation.
Source, binary and log digests are in the source lock. The remaining fault
cases and source-T0/child ownership work stay open.

The pinned `CookieManager::SetCanonicalCookie` also validates and can adjust a
record's expiry before forwarding it to the store. An accepted write is therefore
not, on its own, proof of lossless import. The baseline compares every canonical
member after insertion; production qualification must cover older expiry values
and preflight any unsupported normalization before writing the child. A later
read-back mismatch is not an atomic refusal or rollback.

Likewise, the prototype's deadline and owner-loss handling end the observer-side
operation, not an already-sent engine mutation. Late callbacks are ignored, but a
write may already have been applied or still be in flight. Mutation failures,
read-back mismatches and timeouts require discarding the affected disposable
profiles; clearing the overlap guard is not permission to retry those profiles.
Production child ownership must invalidate the preparation token on such an
outcome and prevent navigation or reuse of that child.

## Source and build fingerprint

- Qt WebEngine release archive SHA-256:
  `6101c1aa00ff933d1b65ee5d167f76e8d71b9ac5b378b0111277723ebda7c163`;
  checked against Qt's published release metadata. Archive size 580351104 bytes.
- Archive `.tag` matches Qt commit `a33fa2a897e5ee58e385b3f88dc247d99fca56db`;
  its Chromium gitlink is `5170777d28bee1ce92cc693a0dbf2ad01492e5cf`.
- Important upstream fingerprint inconsistency: top-level `CHROMIUM_VERSION`
  says base `140.0.7339.264`, whereas `chromium/chrome/VERSION` and generated GN
  defines say `140.0.7339.225`. The security-patch document says `151.0.7922.71`.
  The executed runtime reports Qt/WebEngine `6.11.2`, Chromium `140.0.7339.225`
  and security patch `151.0.7922.71`. Matching the system package's version
  strings does not make this patched library the same artifact.
- Host: two physical cores/four logical CPUs, 15 GiB RAM, approximately 93 GiB free disk after extraction;
  Clang 22.1.8, Python 3.14.7, Node 26.7.0, Qt 6.11.2 development libraries.
- Missing html5lib dependencies are in a workspace-local venv. Missing gperf was
  unpacked locally from its pinned package after a good pacman-key signature
  check. Nothing was installed into system Qt or system Python.
- Configure: Release, jumbo merge limit initially 4, increased to Qt's default 8
  during continuation to reduce repeated header parsing. Inner/outer Ninja began
  at two jobs and was raised to three after observing sustained memory headroom;
  build processes now run at nice level 10. PDF and
  spellchecker disabled, WebRTC enabled, no sandbox changes. Source/build/install
  stay under ignored `target/versioning-engine-spike`.
- Configure warns that SBOM Python dependencies are absent. This research build
  is not a distributable/release-qualified engine.
- Native runs: Linux `7.2.3-arch1-3`, x86_64, Qt offscreen platform, disposable
  off-the-record profiles. The engine used Clang 22.1.8; the standalone harness
  used GCC 16.2.1 with C++20 and `-Wall -Wextra -Werror`.
- [Source lock and artifact digests](../../../tools/versioning-engine-spike/source-lock.json)
  bind the observed runtime to the engine patch, harness, runner, built library,
  helper executable and passing logs. Dynamic-library resolution was checked
  with the runner's local library path and selected the installed experimental
  `libQt6WebEngineCore.so.6`, not the system WebEngine library.

## Checks actually performed

- Official archive checksum and `.tag`: matched.
- Local dependency setup: completed; gperf package signature and SHA-256 matched.
- Qt CMake configure: succeeded with Core, Widgets, Quick and WebRTC enabled.
  The initial missing-html5lib attempt silently disabled WebEngine despite exit
  zero; the runner now explicitly rejects that configuration and selects the venv.
- Qt's GN build tool: built and installed under the local build prefix.
- Chromium GN generation: completed, creating 26409 targets. Full Core build
  initially scheduled approximately 24904 actions; a finished GN generator is
  **not** a finished engine. Build continuation is recorded below.
- Native harness C++20 syntax check against the patched public headers with
  `-Wall -Wextra -Werror`: passed. This does not compile/link the engine extension.
- Cookie delegate and `QWebEngineCookieStore` API wrapper: syntax checks **and
  actual object compilation passed** against the real generated Chromium headers
  and Qt moc output, using the generated GN compiler flags. The check caught and
  fixed the required by-value `CookieAccessResult` callback signature. The local
  objects are `cookie-delegate-probe.o` (75 KiB) and `cookie-api-probe.o` (31 KiB)
  under the spike target directory. This does not link/run the engine or compile
  the separate RTC translation unit.
- RTC observation hook: focused syntax checking and actual object compilation
  passed once the real generated Chromium common headers became available.
  `rtc-hook-probe.o` contains the observer callbacks and adapter method, compiled
  from the corresponding source slice with the generated GN flags and actual
  Qt/Chromium headers. No substitute headers were used. This is narrower than
  compiling the complete adapter translation unit, linking the engine or running
  the RTC fixture. The complete adapter has since also compiled in the main
  build, and the full library has linked and passed the baseline below.
- Shell syntax check: passed.
- Generated install-destination audit: all `file(INSTALL)` destinations use the
  local CMake prefix; helper-link creation does too. The runner supplies that
  prefix explicitly and clears inherited `DESTDIR` packaging relocation.
- Linker fallback preflight: the existing Rust toolchain's GNU-compatible
  `lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld` reports LLD 22.1.8;
  Clang successfully used it to link an empty local shared-library smoke fixture.
  This is only a fallback-tool check. The current engine configuration still
  selects the system linker; no engine link or native capability is proved by it.
- Patch reverse-application dry run with zero fuzz: matched all eight modified
  Qt files. The preparation runner was exercised against the already-patched
  tree and correctly left it unchanged.
- Implementation-pack validator and all pack SHA-256 checks: passed; 96 tasks,
  unchanged statuses (34 verified, 20 in-progress, 6 not-started, 36 blocked),
  no production capabilities enabled.
- Full engine build and local installation: passed. The normal configured
  linker succeeded; the LLD fallback was not needed.
- Native cookie/WebRTC probe execution: **40 checks passed in each of three
  fresh-process runs**, exit 0. Logs under `target/versioning-engine-spike`:
  `native-probe-baseline-2.log`, `native-probe-baseline-3.log`,
  `native-probe-baseline-4.log`. All report
  `PRODUCTION_CAPABILITY_ENABLED=false`. The 40 include fixture/control checks,
  not 40 distinct production guarantees.

### Native outcomes and setup fixes

| Exercised boundary | Observed baseline result |
| --- | --- |
| Canonical copy | Exact 14-to-14 read-back, including all canonical data members and four partition collision variants |
| Unsupported nonced scope | 15-record source refused as a whole; independently read-back destination count 0 |
| Null/self/filter/overlap cases | Refused; ordinary clone still succeeded after removing fixture filters |
| Nonempty destination | Refused; a later fresh-child clone proved that the refusal left the fresh source empty and released the operation guard |
| Owner destruction | Destroying either profile produced OwnerLost once in the bounded fixture |
| RTC construction only | Native positive observation without an offer or ICE candidates |
| RTC data-only pair | Local channel connected, correct-page native observation, no media-permission request |
| RTC controls | Same-profile and other-profile pages remained Unknown; no Clear result was exposed |

Two setup failures were corrected without changing the engine patch or weakening
any assertion. The initial harness build selected the local WebEngine library but
system WebEngine headers: Qt's forwarding headers found the system Qt include
root first. Its CMake target now lists the patched module first, and the generated
include order was checked. The successful install/harness log is
`install-probe-header-order.log`; the original failure remains in
`install-probe-continuation-2.log`.

The first launch then exited 1 before the engine fingerprint or any checks,
because inherited `QT_QPA_PLATFORMTHEME=gtk3` tried to open an unavailable display.
The runner now clears that theme and selects Fusion for this process only while
retaining the offscreen platform. No desktop settings were changed and no
sandbox override was introduced. The failed launch is retained as
`native-probe-baseline-1.log`; it is not counted as a native passing run.

The original 40-check baseline does not exercise real HTTPS cookie request semantics, disk-backed
sources, concurrent writers, injected mutation failures/timeouts/network-service
loss, fresh-child preparation tokens, live merge, multi-frame RTC admission or
race-safe freezing. Those remain the explicit follow-on work packages.

### HTTPS response/request proof — 2026-10-01

The extended patch's `ferricCookieCopyExistingProbe` copied **19 canonical
records exactly** from an owned response/script-populated profile into an empty
off-the-record child before the child's first HTTPS navigation. Three fresh
runs of `probe-https` each passed all 26 checks with exit 0, logged as
`native-probe-https-ancestor-{1,2,3}.log` under the spike target directory.
An earlier 18-record version passed once (`native-probe-https-1.log`) before
the ancestry case was added. The 19-record version is the retained fixture.

The records comprise 13 ordinary response cookies, one script cookie and five
otherwise-colliding partition variants: unpartitioned, first-party/same-site
ancestry, the **same top-site with cross-site ancestry**, top-site A and top-site B.
The ancestry fixture embeds cookie.localhost → top-a.localhost → cookie.localhost;
both it and the direct first-party request select their own distinct record.
The source and copied child both passed real HTTPS request assertions for
host-only versus domain/subdomain scope, path inclusion/exclusion, HttpOnly
script invisibility, prefixes, Strict/Lax/None behavior on a script-initiated
cross-site top-level navigation, and all partition selections. Canonical
read-back equality includes the fields not directly observable in request headers.

The server binds an ephemeral loopback port. The runner generates a one-day
fixture certificate; the page accepts only that exact leaf certificate's
expected authority error at four fixed `.localhost` names and the owned port.
No host mapping, trust-store change, debugger or sandbox override was used.
Chromium emits expected `ERR_CERT_AUTHORITY_INVALID` (-202) diagnostics before
the narrowly pinned test decision; these are retained in the passing logs, not
suppressed or interpreted as cookie failures. Only booleans from request parsing
are retained, never raw cookie headers in logs.

The extension compiled and linked successfully. Its first build invocation
reported a trailing shell EOF error because the launcher was edited while that
invocation was active. The current launcher passed `bash -n`, and a clean build
invocation passed with the completed objects preserved. Do not edit the launcher
while it is running. Installation and both strict-warning harness builds then
passed (`install-probe-https-extension-1.log`).

At that stage, this was not yet package 2's legacy/opaque/expiry/fault coverage, a disk-load or
concurrent-writer source-T0 proof, an ownership token, merge, or RTC admission.
The 40-check regression baseline also passes on the extended engine; its v2
logs and the new hashes are recorded separately from the preserved v1 results.

The later edge fixture adds a separate pair of disposable off-the-record
profiles. Real HTTPS response setters yield exactly two accepted records: a
short-lived valid cookie and a far-future cookie whose observed source expiry
falls within 390–401 days, consistent with Chromium's cap rather than the 2099
header. An expired cookie and invalid Host/Secure prefix, insecure SameSite=None
and insecure Partitioned setters are absent from both source and child requests.
The prototype copies the two accepted canonical records exactly to the untouched
child; the 33-check HTTPS run passed with zero failures. This tests browser
response rejection and source-side expiry normalization, **not** an injected
legacy record, older expiry copied without normalization, network-service loss,
timeout atomicity, opaque partition keys or all production snapshot ownership.
The current source/binary/log set is preserved locally as
`target/versioning-engine-spike/baseline-2026-10-01-v5-edge.tar` with its digest
in the source lock.

### Build continuation history

The main initial build was interrupted deliberately to apply the two-job limit
to Chromium's inner Ninja as well as outer CMake. The next chunk was stopped to
prioritize the focused cookie compilation checks. Generated headers and all
incremental build artifacts were preserved. Following the owner's continuation
request, the full build was resumed under active supervision with a 43200-second
ceiling (initial log: `target/versioning-engine-spike/build-resumed.log`, continued
with Qt's default jumbo size in `target/versioning-engine-spike/build-jumbo8.log`);
three-job continuation is logged in `target/versioning-engine-spike/build-jobs3.log`.
That continuation completed 13059 of its 14188 scheduled actions before reaching
the configured 12-hour safety ceiling at 20:20 UTC (exit 124, no compiler errors
reported). The old process group stopped cleanly. Work resumed immediately,
without reconfiguration or deleting objects, in
`target/versioning-engine-spike/build-jobs3-continuation-1.log`; local installation
and the native probe remain sequenced after a successful build. The complete
1078-translation-unit Chromium browser batch is compiled, but the full engine
link and native execution were still pending at that point. This was not a request for another
approval or a new capability certificate. Work must not be handed back merely
at an arbitrary checkpoint. No scheduler is installed.

On the owner's 2026-10-01 continuation request, the previous build was no longer
running. Its continuation log ended after 49 additional actions with warnings
but no compiler errors recorded; installation and native execution had not run.
The preserved build was resumed in
`target/versioning-engine-spike/build-jobs3-continuation-2.log`, again followed by
local installation and the native probe only on successful stage completion.
The checked-in engine patch digest is unchanged.
This continuation has also compiled the patched
`obj/QtWebEngineCore/cookie_monster_delegate_qt.o` in the normal engine build
(action 894) and the complete `obj/QtWebEngineCore/web_contents_adapter.o`
containing the RTC observer (action 943), beyond the earlier focused object
checks. The engine subsequently completed all 1082 actions in that stage, the
bundled 111-action driver target, and the outer 114-action Qt build with exit 0.
The driver was built/installed locally but was not launched. The full engine
build log is `build-jobs3-continuation-2.log`.

During that continuation, the harness gained null/self-destination checks,
source/destination cookie-filter refusals,
nonempty-destination refusal with a subsequent fresh-destination success check,
and destruction of either profile owner. The RTC signalling fixture now waits
for ICE gathering before passing descriptions and reports an explicit signalling
failure. Its bounded waits use real Qt event loops to service deferred deletion.
An additional, separately destroyed RTC fixture checks native observation of a
constructor without any offer or ICE candidate, independent of the connected-pair
fixture and the scheduler-feature flags discussed below.
Both RTC cases also check a second page in the **same** profile, as well as a
different-profile control, so a profile-wide observation cannot pass as exact-page
attribution.
These are test-source improvements, not new native passing results. The previously
missing autofill Mojo header is now generated, and the complete RTC adapter
translation unit has compiled. The finished engine has linked and run the native
baseline described above.
The extracted-hook check initially needed `ipc/constants.mojom.h`, then
`content/public/common/extra_mojo_js_features.mojom.h`. Once the main build
generated those dependencies, the focused syntax check and object compilation
both passed (`rtc-hook-syntax-generated.log`, `rtc-hook-compile.log` and
`rtc-hook-probe.o`). The earlier missing-header logs remain available, including
`rtc-hook-syntax-retry.log`. The main build continues to generate and compile the
remaining dependencies in order.

The full build was a substantial native build job, not a missing
research permission or proof of unsupported Chromium behavior. The `baseline`
runner executes preparation, configuration, build, local installation and native
probes consecutively, stopping on an actual stage failure. Individual stages can
be resumed with `build`, `install-probe`, then `probe`; the build now defaults to
a 43200-second safety ceiling rather than a 900-second checkpoint. Never start a
second build in the same output directory while one is active. The compiled cookie objects are useful implementation evidence,
and the executed baseline now adds positive evidence for its precise fixture
scope. Neither is a full production capability certificate.

## Qualification work packages, in order

1. **Build and run the baseline — completed 2026-10-01.** Three native runs
   passed all 40 checks. Preserve this as a regression baseline and repeat it
   after engine/patch changes. Full canonical equality, including timestamps
   and source attributes, remains required; unsupported nonced scope left the
   child empty. Do not repeat the initial long build unnecessarily: use its
   preserved incremental artifacts for the next isolated extension.
2. **Cookie representation/request proof.** Use loopback HTTPS, real response
   setters and third-party frames to test host/domain scope, all SameSite policies,
   HttpOnly, real partition top-site/ancestor collisions and actual requests.
   Test opaque as well as nonced keys, prefixes, unsupported legacy source
   metadata, expiry (including setter clamping), rejected writes and callback
   timeout/network-service loss.
   The 19-record/26-check HTTPS fixture qualifies the concrete response/script
   and request cases above, including real top-site/ancestor collisions. The
   33-check edge fixture adds source-side expiry and invalid response setters.
   Synthetic whole-list refusal covers injected legacy-provenance and
   overlong-expiry candidates, not real legacy source records. V7 additionally
   covers refusal of an actual opaque-key source record in a disposable store,
   not a lossless import or general browser-created opaque-key path. Complete
   actual legacy/older-expiry-import/fault cases and broader opaque-key
   qualification before
   marking this work package complete.
   Pinned-source inspection after v8 rules out a tempting shortcut for the
   legacy fixture: `net::CookieMonster::SetCanonicalCookie` calls
   `CHECK_IS_TEST()` for `CookieSourceType::kUnknown` on this Linux build
   (`src/3rdparty/chromium/net/cookies/cookie_monster.cc`, around line 1802).
   Do not send a fabricated unknown-source cookie through the live Qt network
   service setter; it may fatally terminate that service. The 800-day candidate
   likewise cannot be qualified through an ordinary sanitized setter:
   `CanonicalCookie::IsCanonical` rejects expiry beyond its adjusted maximum,
   whereas `IsCanonicalForFromStorage` deliberately permits older stored
   records (`canonical_cookie.cc`, around lines 269 and 960). Chromium's own
   `FromStorageCookieCreated300DaysAgoThenUpdatedNow` test demonstrates the
   distinction (`cookie_monster_unittest.cc`, around line 8343). The next
   disposable proof must exercise a controlled backing-store load or an
   equivalent source-owned migration fixture, then observe the real complete
   source callback and whole-list preflight. Do not infer Qt-profile import
   from the earlier in-memory substitution or use copied Chromium SQLite
   files as a shortcut. This is a source-level constraint, not a new passing
   native test or a production capability certificate.
   The `probe-legacy` harness now creates a target plus safe sibling in each of
   two isolated persistent Qt profiles. A separate native control process
   reloads each ordinary two-record source and copies both to a fresh child.
   After that process exits, the runner changes only fixed fields in each
   closed disposable backing store and reopens it through Qt. One target has
   stored `kUnknown` provenance; the other has a 600-day creation-to-expiry
   span while remaining unexpired. Three fresh full runs on the pinned engine
   passed 16 checks each: both control copies returned two records, while both
   altered two-record sources returned `UnsupportedRecord` with zero child
   writes. This is real native disk-load/refusal evidence, not the earlier
   in-memory substitution. The runner issues no SQLite `SELECT` or row export
   and does not touch a user profile. Direct database mutation is only
   research-fixture preparation, not an implementation path for Ferric. The
   exact source, executable, engine and runner digests and three retained
   disposable directories are in `source-lock.json`. This result does **not**
   qualify migration from a historical Qt profile, legacy import, lossless
   older-expiry copying, network-service faults, source-T0/child ownership,
   or production Ferric integration.
   The pinned `CookieMonster` also exposes a deprecated `SetAllCookiesAsync`
   implementation that inserts canonical records without the ordinary setter's
   checks, but it replaces the destination jar wholesale and is not exposed by
   Qt's current `CookieManager` interface. This is a source-level candidate for
   a *different*, tightly owned experimental seed path, not evidence of a safe
   import API. Any next probe must first prove an empty, exclusive child and a
   frozen source snapshot, then show exact native read-back and failure cleanup
   for the disk-loaded legacy/older-expiry records. Do not call the deprecated
   method on a live profile or promote it based on this inspection alone.
   Follow-on pinned-source inspection narrows that candidate. Qt's persistent
   network context constructs a `net::CookieMonster` over its
   `SessionCleanupCookieStore` (`services/network/network_context.cc`, around
   lines 746–758 and 2883–2897). `network::CookieManager` holds only the base
   `net::CookieStore*` and its Mojo interface has no whole-list seed method
   (`cookie_manager.h`, around line 164; `cookie_manager.mojom`, around lines
   403–447). `CookieMonster::SetAllCookiesAsync` invokes a synchronous
   delete-all/insert-all operation after loading; each inserted persistent
   canonical record reaches `store_->AddCookie` through
   `InternalInsertCookie(..., sync_to_store=true)` (`cookie_monster.cc`, around
   lines 515, 1667–1690 and 1956–1993). This is why a lossless legacy seed is
   plausible in a different backend even though the ordinary Qt setter fails.
   It is **not** safe to expose that deprecated operation directly: it deletes
   existing cookies and does not itself establish child exclusivity or block
   concurrent writers. The next disposable source-pinned prototype should add
   a dedicated, bounded *seed-if-empty* operation on the cookie-store sequence,
   accept only a freshly owned child, reject nonempty/changed generation before
   any mutation, and return count/status only. It must then prove exact native
   read-back of the two disk-loaded edge records, refusal/cleanup on injected
   setter or service loss, and that a competing writer cannot interleave. No
   production flag or general-purpose Mojo method is authorized by this note.
   A separate research-only source overlay now implements the first bounded
   test of that idea. A CookieMonster-sequence callback refuses a nonempty
   destination (including partitioned cookies) before calling the existing
   full-canonical insertion; the network-service entry point caps the list at
   64 records and 512 KiB of name/value/domain/path bytes. A Qt bridge holds
   exact source/destination QObject generations and performs native read-back;
   the harness uses disposable persistent children. This is still **not** an
   exclusive writer lease, an atomic persistent transaction, or a shipping
   interface. The overlay passed its zero-fuzz reverse patch check and the
   isolated rebuild. Three fresh `probe-seed` runs passed 40 checks each with
   zero failures. Each of the real disk-loaded unknown-source and older-expiry
   sources was refused by the ordinary setter, then seeded to a fresh
   persistent child with exact in-process canonical read-back. A fresh process
   subsequently saw two canonical records in each child, and the edge record
   still caused whole-list refusal on an ordinary copy. This is a normal-close
   persistence witness, not a member-by-member canonical comparison across
   processes. Qt's `cookieAdded` name signals were **not observed** on those
   reopened children, even with the listener registered before page creation;
   the fixture records that separately and does not treat signal absence as
   evidence that the native cookie jar was empty. See `source-lock.json` for
   the overlay, binary and fixture digests and retained disposable paths.
   The pinned insertion path calls `SQLitePersistentCookieStore::AddCookie`,
   whose backend batches writes for up to 30 seconds or 512 operations unless
   explicitly flushed. The seed callback and immediate `GetAllCookies`
   read-back therefore prove an in-memory state, not durable publication. The
   fresh-process stage witnesses normal-close persistence only; a production
   protocol still needs a qualified flush/failure contract and crash/restart
   tests before exposing a child as durably seeded.
   The source-pinned `FlushStore` callback is also insufficient on its own:
   `SQLitePersistentStoreBackendBase::FlushAndNotifyInBackground` calls
   `Commit()` and posts an untyped completion regardless of whether the
   cookie store's `transaction.Commit()` succeeded. The current backend logs
   commit problems but does not return an insertion/transaction outcome through
   that callback. Production would need a different status-bearing durability
   witness or must keep the child unavailable on uncertainty.
   The negative stage prepared a disposable child with one ordinary marker
   before modifying either source fixture. The initial generic Qt preflight
   rejected it before the engine guard, so the research seed mode was adjusted
   to reach the engine's seed-if-empty entry. In all three passing runs that
   entry rejected the nonempty child; an independent ordinary copy into a
   fresh off-the-record control still found exactly its one marker. This tests
   the engine's nonempty guard without touching a user profile, but does not
   establish an exclusive writer lease or concurrent-writer failure cleanup.
   A further source check found that the existing `CanonicalCookie` Mojo
   deserializer calls `CanonicalCookie::FromStorage`, not the ordinary sanitized
   setter (`services/network/public/cpp/cookie_manager_mojom_traits.cc`, around
   lines 739–803). The already-passing disk-loaded refusal probe also received
   these records through native `GetAllCookies`. This removes one suspected
   transport-format obstacle, but does not prove destination admission,
   persistence, writer exclusion, or lossless seed read-back.
3. **Source T0 and child ownership.** Add a bounded native snapshot handle tied
   to exact profile/network-context generation. Prove the pinned single-sequence
   `GetAllCookies` boundary during disk loading, response/script/service-worker
   writers and expiry. Enforce a fresh child preparation token and no navigation
   or competing mutation before successful read-back. Empty-list checks alone
   do not provide that ownership. Only then integrate with VB-073/074/075 and the
   existing application seed contract; invalidate evidence on engine updates.
   A further source-pinned inspection narrows the T0 question:
   `CookieMonster::GetAllCookiesAsync` enters `DoCookieCallback`, which starts
   the complete store load when needed and queues the global callback until
   loading finishes. Its synchronous `GetAllCookies` then removes expired
   entries, copies both ordinary and partitioned cookie maps into a separate
   `CookieList`, and invokes the callback on the CookieMonster thread. This is
   a plausible complete per-store point-in-time cut, stronger than Qt's
   `loadAllCookies` signals. A separate research-only overlay now forces one
   native setter after that callback but before seed. Three fresh runs passed
   seven checks each: the source advanced from two synthetic cookies to three,
   while exact child read-back and a second independent copy still found the
   captured two. The 40-check seed and 53/33/59/59 baseline regressions passed
   on the overlaid engine. This proves the bounded post-callback ordering case,
   not a Ferric source-T0 certificate: the current Mojo result has no exact
   profile-owner generation/lease, expiry collection itself changes the store,
   and unforced response, script or service-worker writers are not excluded.
   Test writes on both sides of the callback, plus owner loss and concurrent
   writer ordering, before using this boundary for a real branch.
   The current `CookieManager`-local `ferric_probe_seed_busy_` flag is not a
   child writer lease. In this pin, both `CookieManager::SetCanonicalCookie`
   and `RestrictedCookieManager` dispatch directly to the same
   `CookieStore::SetCanonicalCookieAsync`, while HTTP response handling in
   `URLRequestHttpJob` calls `CookieMonster::SetCanonicalCookieAsyncAndFiltered`
   without passing through the general CookieManager method. Those routes
   converge inside CookieMonster, which also exposes deletion operations and
   mutates expiry/access metadata. Guarding only the new Mojo seed entry would
   leave response, script/worker and deletion writers unexcluded. A production
   lease must either interlock the CookieMonster/store mutation boundary with
   explicit failure semantics, or establish by construction and native stress
   evidence that an unpublished fresh child cannot have any of those writers.
   A new research-only page-free child fixture tests the latter direction.
   Constructing `QWebEngineProfile` and calling `cookieStore()` alone left the
   delegate without a bound CookieManager; the native seed returned
   `Unavailable` with source/destination counts zero in two disposable runs.
   The separate overlay explicitly calls the profile's default storage
   partition and browser-process cookie manager without constructing a
   `QWebEnginePage`. Three fresh runs then passed 46 checks each: both
   disk-loaded edge sources seeded exact two-record children before page
   creation, an in-process post-publication read still found two native
   records, and a separate process found two after normal close. The previous
   seed, cookie, HTTPS and both RTC fixtures passed 40/53/33/59/59 checks on
   this engine. The source-pinned hook is only in the disposable build.
   Absence of an application page reduces renderer/HTTP response writers by
   construction in this fixture; it does **not** establish that the network
   context has no browser-process writer, that a competing Qt/native caller
   cannot mutate the jar, or that no writer can race its empty check and seed.
   A production protocol still needs an exclusive mutation boundary or a
   stronger proved unpublished-owner invariant, plus source-owner generation,
   status-bearing durable commit, fault cleanup and multi-context tests.
   In particular, Qt's `ConfigureNetworkContextParams` supplies a new cookie
   manager to the profile delegate, whose setter first unbinds any old manager.
   A stable Qt store pointer is therefore not a network-context identity proof.
   The prototype guards rebinding with a generation, but restart/rebinding and
   explicit storage-partition identity still need qualification; no multi-context
   routing guarantee is inferred from the fresh, single-context fixture.
   The pinned Qt delegate's `hasCookieMonster()` checks only whether its Mojo
   remote is *bound*; it does not prove the pipe is still connected. On
   `ConfigureNetworkContextParams`, Qt installs a fresh remote and its setter
   unbinds the previous one, which the experimental generation guard sees.
   A network-service disconnect without a successful rebind can instead leave
   a bound but unusable remote and a callback that never arrives. Production
   qualification therefore needs an explicit disconnect/timeout transition to
   `Unknown`, with owner generation revocation and failure cleanup, rather
   than treating `hasCookieMonster()` as a liveness certificate.
4. **WebRTC admission protocol.** Use browser observation for positive refusals,
   but put absence/admission in a renderer/browser check-and-freeze protocol.
   Check every live local/OOP frame against exact document/generation identities;
   cover creation during admission and in visibility/freeze callbacks, frame
   creation/navigation/detachment, crashes and missing acknowledgements. Freeze
   acknowledgement alone is not proof that no connection was already present.
   All unresolved intervals return Unknown and restore any prepared views.
   V3 now passes the bounded frozen-registry witness cases above, including
   local/OOP children and freeze-handler microtasks. V4/V5 add a bounded
   experimental lease and tested Qt app-script dispatch exclusion. Production
   ownership, all media/capture categories, other script sources and remaining
   fault cases are still required before this work package or strict stash can pass.
   A further pinned-source inspection found a possible bounded activity input:
   Qt 6.11.2's `WebContentsDelegateQt` maintains per-delegate counts for device
   audio/video, tab mirroring, and desktop/display capture, including the
   `DISPLAY_VIDEO_CAPTURE_THIS_TAB` and `DISPLAY_VIDEO_CAPTURE_SET` variants.
   `WebContentsAdapter::determineRecommendedState` already checks the four
   positive-count predicates before recommending Frozen. These counts are a
   candidate **positive refusal** and eventual admission cross-check, not a
   standalone `Clear` certificate: they do not account for data-only peers,
   pending permissions/stream creation, missed callbacks, or the interval from
   the last count read to a retained freeze. A future native fixture must race
   capture start/stop and callback loss against the exact-view lease before
   combining these counts with the renderer registry witness.
   The pinned `removeDevice` decrements its plain integer counter without a
   nonnegative assertion or saturation. Its `MediaStreamUIQt` caller normally
   guards duplicate starts and removes only after a start, so this is not an
   observed underflow. An unexpected negative count would still be Unknown,
   not Clear. More importantly, the counter is incremented in `OnStarted`,
   after access-request handling, so pending capture permission or stream
   creation is outside that positive-count witness. Qualify request/start/stop
   ordering and callback loss natively before using it for admission.
   The dispatcher keeps `m_pendingRequests` per `WebContents` during permission
   handling, but `handleMediaAccessPermissionResponse` removes a request from
   that queue before invoking the media response callback. `MediaStreamUIQt`
   increments the delegate count only on its later `OnStarted`. Thus a simple
   `pending queue empty && active count zero` test has a concrete admitted-
   but-not-started gap. A future lease needs an explicit in-flight handoff
   witness across that interval, not just both existing observations sampled
   together. The desktop-capture path with a preselected device ID is a
   separate bypass: `processMediaAccessRequest` calls
   `processDesktopCaptureAccessRequest` before enqueueing, and that helper
   returns a successful media response directly. An absence protocol must
   interlock this direct path too; instrumenting only `m_pendingRequests`
   cannot cover it.
   A follow-on research overlay now wires one bounded request token before
   either dispatcher branch. The token remains pending across permission
   response and is handed to `MediaStreamUIQt`; `OnStarted` increments the
   existing active counters before retiring the token, while denial, queue
   removal, or UI destruction retires it without claiming a stream started.
   New capture requests synchronously invalidate an in-progress or retained
   experimental lease before routing, and the lease candidate refuses if any
   per-delegate pending, active device-audio/video, mirroring, or desktop count
   is nonzero or otherwise not exactly zero. A 512-request saturation poisons
   that diagnostic count to non-clear rather than wrapping. This wires the
   preselected-desktop fast path through the same token because counting begins
   before its early branch. The native fixture authors delayed microphone
   permission, active fake-stream, denial, and stop/release cases using one
   fixed fake device flag. A separate test-only native seam submits an invalid
   selected desktop ID while an empty lease is retained and requires
   synchronous lease invalidation plus native refusal. The expanded fixture
   also calls the real page-side legacy desktop `getUserMedia` path with
   `screen:-3:0` on an owned HTTPS page. A separate disposable engine overlay
   attaches a bounded synthetic frame generator only to Chromium's fake-ID
   desktop capturer, allowing the native probe to await device start, one
   synthetic video frame, active-lease refusal, and cleanup after track stop
   without selecting or reading an OS screen. This is an authored source path,
   not yet runtime evidence: both overlays apply with zero fuzz in order to a
   temporary extraction of the pinned Qt package after the existing
   capability/seed patches, but neither overlay nor fixture is **built or
   run**. Even if that narrow route passes, it remains neither complete
   renderer/WebRTC absence proof nor a production capability.
5. **Merge, independently.** Qualify revision/precondition checking, writer
   control covering responses/scripts/workers/other tabs, exact mutation results
   and final read-back. The proposal allows partial/unknown results; no whole-jar
   rollback is promised. `SetAllCookiesAsync` is deprecated and is not a safe
   replacement for this protocol. In this pin its replacement loop clears only
   the ordinary-cookie map, not the partitioned map, and it returns a default
   access result rather than individual insertion outcomes. Its direct insertion
   path is therefore not a qualified workaround for legacy provenance either.
   A successful clone does not unblock merge.
   Do not build an alleged critical section by nesting ordinary async operations
   inside the initial `GetAllCookies` callback: `InvokeQueue` marks all-cookie
   loading finished only **after** draining the queued callbacks, so nested
   operations can themselves be queued. Any store-sequence conditional/batch
   extension must define and test its actual mutation boundary rather than rely
   on callbacks happening inline in an already-warm fixture.
6. **Integration/release.** Only scoped native evidence can unblock dependent
   implementation slices. The probe uses `QWebEnginePage`; Ferric's Qt Quick
   view bridge, opaque ownership/generation handles and Rust adapter integration
   are not supplied by this patch and must follow qualification. Production
   still requires end-to-end journeys,
   security/licensing/update/rollback review and the final acceptance campaign.

### RTC follow-on: existing renderer registry and freeze machinery

Further pinned-source inspection narrows the follow-on implementation route:

- `PeerConnectionTracker` is a **per-LocalDOMWindow** supplement, not a global
  per-process absence counter. Production handler initialization obtains that
  tracker and inserts each successfully created native connection into its local
  registry before returning. Unregistration removes the entry. Ordinary close
  and unregistration are distinct: a closed handler can remain registered.
  A small, content-free renderer query over this existing registry is a candidate
  for a document-bound witness; do not call its verbose stats/state APIs or infer
  liveness solely from registry size.
- The existing `kWebRTC` / `kWebRTCSticky` scheduler flags are **not registered
  for every constructor** in this pin. `DisableBackForwardCache` is called when
  remote descriptions contain candidates or when adding ICE candidates. These
  flags therefore cannot certify absence of newly created or negotiating peers.
  Moving registration to construction would not, by itself, make the existing
  browser reports exhaustive: `BFCacheBlockingFeatureAndLocations::MaybeAdd`
  caps each sticky/non-sticky detail list at ten entries. The renderer sends
  those lists, and `RenderFrameHostImpl::GetBackForwardCacheDisablingFeatures`
  reconstructs its feature set from the reported details. An omitted entry is
  therefore another possible missing-positive path. The renderer's complete
  internal feature-count map is not what that browser getter returns. Do not
  substitute these bounded diagnostic reports for a dedicated absence witness:
  later removal of reported entries does not backfill previously omitted ones,
  so a short report is not proof that truncation never occurred.
- `WebContentsImpl::SetPageFrozen` visits the primary tree's RenderViewHosts.
  Within each renderer, `WebViewImpl` dispatches lifecycle changes and flushes
  scheduler-feature reports before acknowledging the update. This provides
  existing coordination machinery to investigate, not an RTC absence witness:
  the acknowledgement has no connection-registry result, and the feature flags
  have the coverage limitation above.
- The current peer-manager receiver uses `kMiscPlatformAPI`, whose pinned
  `FrameSchedulerImpl` traits explicitly allow freezing and whose queue is
  disabled when its page freezes. A post-freeze query on that receiver is not a
  usable admission barrier. `WebViewImpl` instead binds its page-lifecycle
  receiver to the agent-group default runner. A candidate extension can carry
  the witness on that lifecycle/control path; its actual frozen-state behavior
  still needs native tests.
- Blink already has a `CoreInitializer` / `ModulesInitializer` virtual boundary
  used from lifecycle code. That is a candidate seam for a content-free registry
  query without adding a direct core-to-peerconnection-module dependency.
- A coarser alternative is the existing renderer-wide
  `InstanceCounters::kRTCPeerConnectionCounter`, but it is **unsafe as-is** for
  admission. The ordinary constructor path increments before native-handler
  initialization; however, its fenced-frame refusal can return before that
  increment, while destruction unconditionally decrements. `Create` has already
  allocated the garbage-collected object before checking that exception. This
  is a source-visible accounting hazard, not a reproduced native failure: a
  counter-based route must first audit/repair all construction/destruction paths
  and test rejected construction followed by garbage collection. Merely rejecting
  negative readings would not rule out an undercount that happens to read zero.
  A subsequently qualified **zero-only** witness might avoid a per-document
  registry bridge, but the relaxed counter is not a liveness API or synchronization
  barrier. Native-handler shutdown and control-thread ordering also need proof.
  Nonzero cannot identify this page and
  can include failed or closed-but-retained objects and other pages, so it must
  conservatively refuse admission rather than report an exact-page observation.
  A zero read still needs every source renderer, exact frame/document generations
  and proved freeze ordering. Do not expose the testing JavaScript accessor.
- Qt recommends `Active` when the site instance reports more than one related
  active WebContents, but its lifecycle setter does not enforce that particular
  recommendation. The admission protocol must account for related/opener
  windows explicitly; merely calling the setter is not an exclusion guarantee.
- Qt's `QWebEnginePagePrivate::runJavaScript` refuses `Discarded`, not `Frozen`;
  the adapter forwards a valid frame's request to Chromium without its own
  Frozen-state exclusion in the pinned upstream source. The first v4 native
  lease test **did** observe that request execute while reported Frozen. The
  experiment now revokes its lease before dispatch, but already queued/in-flight
  requests still need separate admission tests and an ordering boundary.

The v3 prototype binds a bounded per-document registry witness to the exact
frame set and native frozen state using the separately bound control-task path.
The scoped cases recorded above pass, including freeze-handler microtasks, local
and OOP frames, closed-retained objects, frame changes and related-window refusal.
V4 retains a bounded experimental ownership lease and revokes it for new app
script dispatch. V5 additionally refuses while explicit Qt application scripts
are still pending, including no-callback child scripts; neither covers all
renderer-side activity or loss of engine acknowledgements. A
production protocol must combine all required media/capture activity and qualify
negotiation/admission races, missing/stale acknowledgements, renderer loss and
other queued/in-flight script sources. Missing coverage or identity must
remain Unknown. The implemented historical witness is **not permission to expose
production Clear**; the v4 lease is also not a production admission certificate.

## Qt Quick exact-view bridge prototype — 2026-10-04; focused probe passed

Inspection of the pinned Qt Quick source showed that `QQuickWebEngineViewPrivate`
owns the same `WebContentsAdapter` used by the existing `QWebEnginePage` probe,
but the research API existed only on the Widgets-facing page object. The
disposable `qt-6.11.2-rtc-quick-bridge-probe.patch` now adds QML-invokable
acquire/check/release methods and result/invalidation signals to the actual
`WebEngineView` QML type. The adapter lease is tied to that exact QML item,
requires an off-the-record profile, is released on lifecycle exit, profile
adapter replacement, or view destruction, and is invalidated by the existing
script-dispatch path before Qt dispatches the script.

The authored `rtc_quick_bridge_probe.qml` fixture hides the loaded QML view,
obtains an empty-registry lease, dispatches a script that constructs an
unnegotiated data channel, requires synchronous lease invalidation before the
script callback, and then requires the exact QML view's next registry read to
report a registered peer and refuse an empty lease. The runner selects only the
fixed fake-device switch and an off-the-record profile. No normal profile,
credentials or OS capture device are used.

The bridge review identified asynchronous-owner hazards: overlapping acquire
calls could overwrite the first release closure, and the view could leave
Frozen before its callback installed a returned lease. The method now refuses a
second in-flight acquire and rechecks the exact adapter, hidden visibility and
Frozen state before retaining a lease. Teardown advances an owner generation so
late completions release their native leases rather than mutating a replaced
owner; adapter replacement also emits invalidation only after the new adapter
is installed, preventing signal-driven re-entry through the old one.

The overlay was checked with zero fuzz against the pinned Qt Quick source,
built, and passed one fresh runtime run. The QML fixture confirmed empty
admission on the exact hidden `WebEngineView`, duplicate-acquisition refusal,
lease invalidation before app-script dispatch, and positive observation/refusal
for the data-only peer created by that script. This closes the research
prototype's Widgets-versus-Qt-Quick owner mismatch for this exact path. It does
not qualify all renderer creation/negotiation races, every media/device
category, loss/fault paths, or Ferric's production Rust ownership, and does not
authorize a shipping Qt fork.

The Qt Quick fixture was then extended to request camera-only
`getUserMedia({video:true})` on that exact QML view, grant only the corresponding
fake-device media permission, require pre-dispatch lease invalidation, observe
a synthetic frame, require fail-closed admission while its track is live, and
wait for a fresh empty lease after stopping the track before repeating the
data-only peer check. It uses Chromium's fake media device and does not touch an
OS camera. This latest QML fixture has not been built or run; the recorded
Qt Quick pass applies to the prior fixture digest kept in the source lock. No
camera-route capability or production status is claimed.

The same exact-view fixture now also holds an audio-only fake microphone
permission pending while it requests an empty-registry lease. It requires
refusal before granting the synthetic permission, then checks that the live
audio track also refuses an empty lease and that stopping the track restores
empty admission. The held permission ensures the pending-media interval is
exercised on the actual QML owner before capture begins. This latest QML
fixture remains authored but unbuilt/unrun; it uses no OS microphone and adds
no production capability claim.

The current QML fixture also makes the exact hidden `WebEngineView` visible
again and requires its native Frozen lease to be revoked on transition back to
`Active`; after hiding the same object, it must obtain a fresh empty-registry
observation. This lifecycle leg is authored but unbuilt/unrun, so the prior
one-run exact-owner result still applies only to the older fixture digest.

Review of the isolated QML bridge found its lifecycle-state handler checked
for an installed lease but not an asynchronous acquisition still pending. The
research-only patch now invalidates either state when the exact view leaves
`Frozen`. The fixture adds a second acquisition immediately followed by an
`Active` visibility transition, accepts either the explicit invalidation or a
late callback refused by the exact-owner lifecycle recheck, then hides the
same item and requires fresh empty admission. Neither the latest patch nor
fixture has been patch-checked, built or run; prior evidence remains bound to
the earlier source and fixture hashes. No shipping Qt change or production
capability is authorized by this experiment.

The latest QML fixture also swaps the exact leased view to a second
off-the-record profile, requires adapter-replacement invalidation and page
reload, then requires a fresh empty observation on the replacement adapter.
This is authored but unbuilt/unrun; existing one-run evidence still binds to
the prior patch and fixture digests. Both profiles are disposable and no
credentials or OS capture devices are involved.

The same QML fixture now adds a combined synthetic audio/video
`getUserMedia` route. It holds the combined Qt permission pending, requires
pending and active refusal on the exact `WebEngineView`, checks live audio and
video tracks plus an actual synthetic video frame, stops both tracks, and
waits for fresh empty admission before the data-only peer probe. This fills a
distinct combined-permission/counter path not covered by the separate camera
and microphone legs. It is authored only: the latest fixture/patch tuple has
not been patch-checked, built or run, so no passing result includes it.

## RTC media-activity and exact-view probe results — 2026-10-04

The disposable capability, seed-if-empty, pending-media admission,
synthetic fake-desktop and Qt Quick overlays now apply in order with zero fuzz
to a fresh extraction of the pinned Qt archive. The pending-media admission
and fake-desktop overlays were compiled into the pinned engine. One fresh native run and one fresh
strict-site-isolation run passed the focused RTC fixture, including pending
permission refusal, fake-audio start/stop, denial cleanup, simulated dispatcher
handoff and counter saturation/overflow/underflow refusal. The real
`screen:-3:0` fake-source route started capture, delivered a nonzero synthetic
frame read directly from the captured track, refused empty admission while the
track was live, and cleared the interlock after stop. Both runs printed
`PRODUCTION_CAPABILITY_ENABLED=false`. The strict-isolation fixture also
observed the expected separate renderer processes for cross-origin frames.

The first frame assertion used `requestVideoFrameCallback`, but the offscreen
compositor did not provide a reliable presentation callback. That was replaced
with `MediaStreamTrackProcessor`, which checks the actual track frame dimensions;
the corrected fixture passed in both configurations. This removes the
offscreen-compositor dependency from the assertion without suggesting that the
first engine route itself failed.

These focused results show that a source-pinned, disposable Qt WebEngine change
can expose the tested WebRTC registry and media/capture activity through the
actual Qt Quick owner. They are not the final multi-run campaign or a production
capability certificate. Other renderer-side creation paths, media/capture
categories, missing acknowledgements, renderer loss, profile/window ownership,
and the Rust application handoff remain unqualified. VB-033 and dependent
production capabilities therefore remain blocked/disabled pending those gates
and explicit shipping-backend/release qualification.

## Synthetic camera-route fixture extension — 2026-10-04; authored, not run

The native RTC fixture now additionally requests camera-only
`getUserMedia({video:true})` against Chromium's fixed fake media device. It
holds the public video permission pending and requires the lease probe to
refuse before grant; after grant it waits for a live track and a nonzero frame
read from `MediaStreamTrackProcessor`, requires refusal while the fake camera
track is active, then stops the track and waits for a fresh empty-registry
lease. This does not access an OS camera or qualify physical-device behavior.
The fixture was extended after the recorded default/OOP runs and has not been
built or run, so no passing result includes this camera route. The source lock
keeps the previously exercised fixture hash separate from the new hash. This
narrows the next research question but does not change the VB-033 blocker or
any production capability.

The native fixture now also exercises renderer-process loss while the exact
`WebContents` holds an empty-registry lease. Its Linux-only helper accepts
only the current probe's `QtWebEngineProcess` renderer child, opens a pidfd,
rechecks process start identity, and signals only through that pidfd. The
fixture requires one lease invalidation and an `Unknown` follow-up result.
This is authored after the last exercised fixture hash and has not been
built or run; it is not yet renderer-loss evidence. The signaling path refuses
to act if it cannot establish the owned disposable renderer identity.

The current native fixture additionally exercises a combined synthetic
camera-and-microphone request. It requires Qt's combined permission type to
remain pending while empty admission is refused, grants only Chromium's fixed
fake devices, checks both live tracks and a nonzero video-track frame, refuses
admission while capture is active, and requires fresh empty admission after
stopping both tracks. A separate fresh off-the-record profile denies the
combined permission and requires the pending token to clear before empty
admission is observed. This is not physical-device coverage. These cases are
authored after the last exercised hash and remain unbuilt/unrun; they do not
qualify production activity absence or change VB-033's blocker.

The exact Qt Quick owner fixture now also closes its data-only peer, reacquires
an empty-registry lease on that same hidden `WebEngineView`, and exercises
renderer loss through the QML-owner bridge. A C++ fixture timer reads only that
view's reported renderer PID; on Linux it signals only after `/proc` identity
checks establish the exact disposable `QtWebEngineProcess` child, rechecks
process start identity after `pidfd_open`, then signals through the pidfd. The
QML case requires Qt's termination signal and exactly one bridge invalidation.
This current fixture is unbuilt/unrun and is not renderer-loss evidence or
production support.

Static review then found an admission-state mismatch in the Quick bridge:
native admission requires a hidden `Active` adapter at entry and initiates its
own transition to `Frozen`, while later fixture continuations reacquire from
hidden `Frozen`. The research-only bridge now explicitly transitions a hidden
`Frozen` owner to `Active` before admission, rejects a visible owner, and still
retains a lease only when the exact owner is hidden and `Frozen` at completion.
The QML fixture now explicitly requires immediate refusal when acquire is
called on the visible owner. The acquire/check/release invokables also reject
foreign-thread calls before reading private Quick-view state. The C++/QML
runner exercises all three while the exact lease is held, then requires it to
remain valid. These source and fixture corrections are not patch-checked, built
or run; all previous runtime results remain bound to their recorded patch and
fixture hashes.

## Qt Quick exact-owner lifecycle continuation — 2026-10-05; one focused run passed

The current Quick bridge patch passed a zero-fuzz reverse-application check
against the pinned Qt source, rebuilt the isolated `QtWebEngineQuick` library,
and loaded it from the disposable install prefix. One fresh
`probe-rtc-quick` process passed the exact-view empty-lease, overlapping-acquire
refusal, foreign-thread guards, hidden lifecycle exit, pending-acquisition race,
profile-adapter replacement, and renderer-loss journeys. The engine capability
and production flags remain disabled.

The data-only peer leg established an important conservative boundary: calling
`RTCPeerConnection.close()` closed the connection but left its peer handler in
Chromium's per-window registration map while the object remained in the live
document. The fixture therefore requires registration to remain visible after
close, reloads that same `WebEngineView` to destroy the old document context,
then acquires a fresh empty-registry lease. This matches the pinned Chromium
`RTCPeerConnection::close()`/`CloseInternal()` path and its separate
handler-unregister-on-context-destruction path; do not equate a closed transport
with no retained peer object.

The renderer-loss route now invalidates the exact Quick owner's held or pending
lease from its `renderProcessTerminated` callback. The owner generation makes a
racing native observer callback stale, so the fixture observed one invalidation.
The Linux test helper accepts only the reported `QtWebEngineProcess` renderer,
checks its renderer role and ancestry through at most two QtWebEngine zygotes
rooted at the fixture process, opens a pidfd, rechecks process-start identity,
and signals through that pidfd. It does not broaden signalling to arbitrary
children or PIDs.

Camera, microphone and combined Quick permission routes remain disabled in
this fixture. Earlier offscreen hidden-view fake capture attempts failed before
Qt's permission callback, so there is no Quick permission/capture result to
claim. Native widget fake-device routes are separate evidence and do not fill
that gap. One focused Quick run is not the final repeated campaign, complete
renderer/media/capture proof, production Rust ownership integration, or release
qualification. VB-033 remains blocked and production stays disabled.

## Pinned source anchors

- [Qt cookie delegate](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/net/cookie_monster_delegate_qt.cpp): discarded completion/set outcomes.
- [Qt network-context wiring](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/content_browser_client_qt.cpp): `ConfigureNetworkContextParams` and cookie-manager binding.
- [CookieMonster](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_monster.cc): `GetAllCookiesAsync`, `DoCookieCallback`, ordinary and partitioned enumeration.
- [Cookie-store contract](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_store.h): sequence ownership and cancellation.
- [Cookie identity](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/cookie_base.h): host-only, partition and source dimensions.
- [Canonical equality](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/net/cookies/canonical_cookie.cc): `HasEquivalentDataMembers` includes all data members.
- [Canonical-cookie Mojo traits](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/services/network/public/cpp/cookie_manager_mojom_traits.cc): canonical-record and partition-key decoding at the internal engine boundary.
- [Network cookie manager](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/services/network/cookie_manager.cc): setter validation and possible expiry adjustment before store insertion.
- [RTC observer](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/content/public/browser/peer_connection_tracker_host_observer.h): frame-bound creation/removal notifications.
- [Renderer connection registry](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/modules/peerconnection/peer_connection_tracker.cc): per-window ownership, registration and control receiver.
- [RTC feature-flag timing](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/modules/peerconnection/rtc_peer_connection.cc): candidate-dependent BFCache flags and close/reset behavior.
- [Native peer handler](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/modules/peerconnection/rtc_peer_connection_handler.cc): initialization, close and unregistration.
- [Renderer instance counters](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/platform/instrumentation/instance_counters.h): renderer-wide relaxed counters are not synchronization barriers.
- [Renderer lifecycle](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/core/exported/web_view_impl.cc): lifecycle transitions can run page callbacks.
- [Frame task-queue policy](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/platform/scheduler/main_thread/frame_scheduler_impl.cc): freezable receiver task types and queue disabling.
- [Scheduler detail-list bound](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/platform/scheduler/public/frame_or_worker_scheduler.h): ten-entry sticky/non-sticky diagnostic lists are not exhaustive feature sets.
- [Browser feature reconstruction](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/content/browser/renderer_host/render_frame_host_impl.cc): reported details feed the browser-side BFCache feature getter.
- [Core/modules seam](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/third_party/blink/renderer/core/core_initializer.h): virtual module services used by core lifecycle code.
- [Browser freeze coordination](https://github.com/qt/qtwebengine-chromium/blob/5170777d28bee1ce92cc693a0dbf2ad01492e5cf/chromium/content/browser/web_contents/web_contents_impl.cc): primary-tree RenderViewHost freeze requests.
- [Qt lifecycle adapter](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/web_contents_adapter.cpp): distinct lifecycle recommendation and transition checks.
- [Qt page script dispatch](https://github.com/qt/qtwebengine/blob/a33fa2a897e5ee58e385b3f88dc247d99fca56db/src/core/api/qwebenginepage.cpp): the wrapper's script-execution refusal checks Discarded state.
- [Qt build/sandbox requirements](https://doc.qt.io/qt-6/qtwebengine-platform-notes.html).

The public-backend negative qualification remains valid. This record and
[ADR-0016](../../architecture/ADR-0016-versioning-engine-capability-backend.md)
supersede the broader inference that the capabilities are inherently impossible.

## Current RTC fixtures repeated — 2026-10-05

The latest native RTC fixture passed three fresh default and three strict-site-
isolation off-the-record runs. The latest Qt Quick exact-owner fixture passed
three fresh runs. The native fixture exercised the synthetic fake audio,
camera, combined audio/video, permission-denial, and legacy fake-desktop routes,
including pending/active refusal, delivered frames, cleanup, bounded-counter
refusal, and renderer-loss invalidation. The Quick fixture additionally
exercised synthetic camera, microphone, and combined permissions on the exact
`WebEngineView`, along with lifecycle/foreign-thread guards, adapter
replacement, data-only peer close behavior, document destruction, and
exactly-once renderer-loss invalidation.

The first current native renderer-loss attempt exposed an overly strict probe
identity rule: Chromium had inserted a `QtWebEngineProcess` helper and had
sanitized its command line. The final helper uses the target PID reported by
the exact `QWebEnginePage`, verifies the isolated executable and bounded
ancestry of at most two intermediate helpers rooted at the fixture, captures
process start identities, opens a pidfd, repeats the ancestry/identity check,
and signals only through that pidfd. This guard passed all six native runs.
It does not use Chromium command-line text as identity evidence.

These are source-pinned prototype results only. Synthetic devices do not
qualify physical devices, the tested categories do not establish universal
RTC absence, and the isolated engine changes do not implement production Rust
ownership or release qualification. The data-only peer remains registered
after `RTCPeerConnection.close()` while the object remains in the live
document. Production capability stays disabled and VB-033 stays blocked. Exact
fixture, binary, and run-log hashes are in
[`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).

## Disposable cookie-copy source/destination owner loss — 2026-10-05

The legacy-cookie fixture now has paired owner-retirement stages for the two
safe, disk-loaded control profiles. `owner-loss` starts an exact-source copy to
a fresh disposable persistent destination, destroys that destination
page/profile before the event loop can route the native reply, then copies the
still-live source to a fresh off-the-record control and requires exact
two-record read-back. The added `source-owner-loss` stage destroys the exact
source page/profile while a copy is pending and requires exactly one
`OwnerLost` completion; the possibly mutated destination stays disposable and
is never reused. The isolated runner keeps both destination directories under
its unique disposable root.

Three fresh disposable campaigns passed with
`bash tools/versioning-engine-spike/run-unpublished.sh owner-loss` after the
isolated apply/build/install stages. Each campaign exercised both synthetic
disk-loaded control variants (unknown-source provenance and older expiry).
For each variant, destroying the exact destination profile while its copy
reply was pending produced exactly one `OwnerLost`; the possibly mutated child
was discarded, then the live source copied exactly to a fresh off-the-record
control. Destroying the exact source profile during a pending copy also
produced exactly one `OwnerLost`. Each campaign reported 32 passing checks and
zero failures. The fixture remained confined to its unique disposable root
and reported `PRODUCTION_CAPABILITY_ENABLED=false`; artifact hashes and roots
are recorded in [`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).

This adds scoped QObject/profile-owner callback-retirement evidence only. It
does not simulate a bound-but-disconnected Mojo network service, establish
source generation or child writer exclusion, validate crash-durable seed
publication, or provide a production cookie capability. Production flags
remain disabled.

## In-flight cookie reply after Mojo pipe disconnect — 2026-10-06

A separate native overlay exercised the lower-level failure case missing from
the owner-loss fixture. While CookieManager.GetAllCookies was pending against
the synthetic disk-loaded source, the probe disconnected the bound
CookieManager receiver and dropped the reply. Three fresh source-loss cases
and three fresh destination-loss cases passed, nine checks each (54 total,
zero failures). Every case completed exactly once with TimedOut. For source
loss, the source and destination counts were both zero; for destination loss,
the source retained two records while the destination count was unknown
(-1), and that possibly mutated child was discarded.

The overlay was compiled into the isolated, source-pinned Qt 6.11.2 /
Chromium 140.0.7339.225 build with security patch 151.0.7922.71. An initial
three-job compile hit memory pressure; after reconfiguring at two jobs,
QtWebEngineCore, Widgets, Quick, and the probe install completed successfully.
The campaign log and source/binary digests are captured in
[source-lock.json](../../../tools/versioning-engine-spike/source-lock.json).
The campaign used an already-cumulatively-patched disposable source tree; a
reverse dry-run against that later overlay state failed on overlapping hunks,
so this entry does not claim that the single overlay was independently
reapplied from its intended intermediate state.

Cookie baseline (53 checks), legacy/older-expiry (16), HTTPS (33), and the
current RTC fixture (107 in both default and strict-site-isolation modes)
also passed. The RTC regression includes synthetic-camera delivery but remains
only scoped fake-device evidence—not proof of complete media/capture absence.
This experiment proves conservative timeout completion for these disconnected
pipe paths; it does not kill or restart the network-service process, test
service rebinding, establish profile-generation or competing-writer exclusion,
prove crash durability or fault-atomic cleanup, or resolve the unknown
destination contents. Production cookie and WebRTC capabilities remain
disabled.

## Cookie writer-route source audit — 2026-10-06; source inspection only

The pinned Chromium source exposes a plausible place for a future engine-side
mutation interlock, but confirms that the current seed overlay is not such an
interlock. `CookieManager::SetCanonicalCookie` and
`RestrictedCookieManager::SetCanonicalCookie` both enter the shared
`CookieStore::SetCanonicalCookieAsync` path, which `CookieMonster` implements.
This is a useful common point for ordinary script and HTTP cookie *sets*.
However, deletion and cleanup use separate `CookieStore` calls (including
`DeleteCanonicalCookieAsync`, `DeleteAllMatchingInfoAsync`, and
`DeleteSessionCookiesAsync`), while the research `seed_busy` flag is local to
the browser-process CookieManager. It cannot exclude RestrictedCookieManager,
HTTP response, deletion, or other store writers. A production lease would need
to cover every mutation on the CookieMonster sequence or prove those writers
cannot exist during the operation; intercepting only the CookieManager wrapper
is insufficient.

This makes a fresh, unpublished, page-free destination a plausible narrower
construction protocol for seed: initialize its storage partition and cookie
manager, keep the exact profile private and without pages/extensions/request
clients, seed-if-empty on the store sequence, verify complete canonical
read-back, and publish the profile only after success. Existing disposable
fixtures establish seed/read-back before page creation, but do not prove that
all internal writers are quiescent, provide a typed durable-publication
certificate, or bind the source snapshot to Ferric's profile generation. The
route is therefore a design candidate, not a qualified production capability.
This entry records static source inspection only; it adds no run, patch digest,
or capability promotion. The pinned source revisions and earlier run artifacts
remain in [`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).

## Qt Quick ApplicationWorld peer probe — passed 2026-10-06

The latest exact-view fixture creates and closes its synthetic data-only
`RTCPeerConnection` objects through `WebEngineScript.ApplicationWorld`, the
isolated world used by Ferric's browser-owned page scripts. The pinned native
peer registry observed the peer, and each application-script dispatch revoked
the held empty-registry lease before execution. Three fresh off-the-record
processes passed with exit code 0. The fixture digest and disposable run roots
are recorded under `experimental_rtc_quick_bridge_overlay` in
[`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).

This establishes that the tested peer-registration path is visible across the
main/application script-world boundary for this Qt 6.11.2 prototype. It does
not qualify arbitrary extension worlds, every renderer/media/capture path,
callback loss, production Rust ownership, physical devices, or release use.
Production capability remains disabled and VB-033 remains blocked.

The same pinned Blink source declares both `RTCPeerConnection` and
`MediaDevices` as `Exposed=Window`; direct dedicated-worker access is therefore
not part of this revision's web API surface. This is a source-level boundary,
not evidence that all renderer/media activity has been observed. The exact IDL
paths and digests are retained in the source-lock entry.

## ApplicationWorld microphone admission probe — passed 2026-10-07

The exact Qt Quick fixture now requests the synthetic fake-microphone stream
from `ApplicationWorld`. Three fresh off-the-record runs passed. In each, the
pending permission refused the diagnostic empty-registry lease, granting the
permission produced an active stream that also refused it, and stopping the
track allowed a fresh empty observation. No physical microphone was selected.
The updated QML fixture digest and disposable run roots are in
[`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).

Together with the prior ApplicationWorld data-only peer test, this confirms the
tested native peer/capture interlocks receive those browser-owned script paths
on this prototype. It still does not prove every media, renderer, or engine
activity path; production Rust ownership and release qualification are open.
VB-033 and production live retention remain blocked/disabled.

## Strict-isolation child-frame ApplicationWorld probe — passed 2026-10-07

The exact Qt Quick owner now enumerates `view.mainFrame.children`, selects the
owned `top-a.localhost` child frame, and calls that frame's
`runJavaScript(..., WebEngineScript.ApplicationWorld, callback)` directly.
Three fresh `probe-rtc-quick-oop` processes passed with Chromium's fixed
`--site-per-process` configuration. Each native result observed the peer as
registered against the exact owning view, with at least two frames and two
renderer processes. Reloading that same view removed the child peer and
allowed a fresh empty-registry lease; the existing renderer-loss tail then
passed. One default-configuration run also passed.

This establishes a concrete implementation route for cross-origin child-frame
activity in the pinned Qt 6.11.2 prototype. Qt documents `WebEngineFrame` as
available since Qt 6.8, `runJavaScript` with a world ID on the frame, and the
QML child-frame list since Qt WebEngine 6.10 ([QML frame API](https://doc.qt.io/qt-6/qml-webengineframe.html)).
It does not establish exhaustive page/frame tracking, popup ownership, all
renderer/media activity, lifecycle races in Ferric's production Rust owner, or
a complete WebRTC-absence certificate. Keep VB-033 blocked and production
live retention disabled until those obligations are implemented and qualified.
The fixture digests and disposable run directories are recorded in
[`source-lock.json`](../../../tools/versioning-engine-spike/source-lock.json).
