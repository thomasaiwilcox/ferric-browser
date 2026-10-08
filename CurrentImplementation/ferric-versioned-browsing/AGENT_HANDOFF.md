# Agent handoff — Ferric Versioned Browsing

## Transfer checkpoint — 2026-10-08

The complete source/documentation worktree is committed on branch
`feature/versioned-browsing`; fetch that branch on the new machine. This is a
**work-in-progress checkpoint**, not a completed feature or qualified release.
The 103-slice ledger now records 52 verified, 4 in progress, 39 blocked and 8
not started. VB-099 is the active slice. `validate_pack.py --next` selects only
new not-started work, so it may return no task while VB-099 is in progress;
resume VB-099 rather than bypassing its acceptance tests. No strict production
capability is enabled. The last full workspace gate and feature-enabled Ferric
build passed before the current native-engine compilation; the final campaign
must be rerun after implementation is complete.

On the old machine, the clean Qt WebEngine 6.11.2 build was deliberately
interrupted for this transfer at Ninja edge 2017/4105. It had **not** linked,
installed or passed native backend acceptance tests. Its 269,594-member source
archive plus exact three-file patch passed two provenance comparisons. Generated
build outputs under `target/`, the source archive, research binaries and local
runtime manifests are not Git-transferable artifacts; manifests checked into
`tools/versioning-engine-backend/` describe this development host only. The
research build's positive probes do not certify the production candidate.

On the new machine, first read `tools/versioning-engine-backend/README.md` and
`docs/testing/versioned-browsing/VB-099.md`. On a qualified matching Linux
x86_64 Qt 6.11.2 development environment, run
`bash tools/versioning-engine-backend/production/build.sh all`, then
`bash tools/versioning-engine-backend/production/verify-and-run.sh check` and
its focused acceptance commands. The builder downloads and verifies the pinned
Qt source archive if absent. It intentionally refuses a different system Qt
runtime/toolchain or changed generated configuration; do not weaken its pins
to make a different host pass. Either reproduce the declared tuple or make an
explicitly reviewed new lock and requalify it. See the backend README for the
exact ordered tests and remaining strict-capability work. A backend maintainer,
security-update SLA and release platform matrix are still undecided.

## Goal

Implement the supported Ferric Versioned Browsing vision described in `PROPOSAL.md`, using `IMPLEMENTATION_BACKLOG.json` as the execution ledger. Preserve the architecture and safety boundaries; do not attempt to make unsupported Chromium behaviour appear supported.

**Current run instruction:** continue feasible slices without routine user checkpoints, but do not promote unqualified strict capabilities. On 2026-10-08 the final complete `cargo xtask check --locked --offline` and feature-enabled browser build passed after the test-only held-open fixture. The focused native branch journey passed through Cargo and five fresh-process repeats with an unanswered child replay socket held beyond the deadline, timeout-specific `failed` before release, hidden native-bound child allocation loss/cleanup and retry, three-page mixed failure/reorder outcomes, exact-profile popups through local redirects, fresh child-owned notifications permission, and confirmed owner teardown. VB-061 still needs partial-created-view/first-view replacement and broader lifecycle/focus evidence; VB-063 still needs certificate and external OAuth-style qualification; VB-060 still needs compositor-visible focus evidence. The 103-task ledger has 52 scoped verified, 4 in progress, 39 blocked and 8 not started. Resume VB-099, the active owner-approved pinned-backend implementation slice; no production capability is enabled. The owner approved pursuing a maintained pinned Qt/Chromium backend for the full vision on 2026-10-08; this is engineering direction, not approval to ship the current diagnostic build or mark strict cookie, WebRTC or ordinary viewless-download guarantees complete. A fail-closed Linux x86_64 development verifier/launcher is under `tools/versioning-engine-backend`. A named maintainer, security-update SLA, target platforms, backend qualification and final campaign remain open. Never mark unverified or capability-blocked behavior as verified, and do not complete the goal while implementation or required evidence remains.

**2026-09-30 owner-authorized capability spike:** research and implement an
isolated, source-pinned native Qt/Chromium prototype for cookie and WebRTC
capabilities, and update the implementation documents with the findings. This
is a narrow exception to the public-API-only/no-engine-fork constraint **for the
disposable experiment only**, not shipping-fork approval. Do not ask again before
continuing these documented native tests. The 2026-10-01 full build and local
installation succeeded, followed by three native runs with all 40 baseline
checks passing in each. The real loopback HTTPS follow-on also passed three
times: 26 checks, 19 exact canonical records, source/child request behavior and
same-top-site ancestor-bit collisions. The 40-check baseline still passes three
times on the extended engine. The v3 native frozen-registry witness now passes
35 checks in three default-mode runs and three strict-site-isolation runs; the
latter measure actual two-process child-frame coverage. Freeze-handler/microtask
connections, frame changes, navigation, visibility, related-opener refusal,
overlap, owner loss and frame-budget refusal are covered. Both cookie suites
also pass three times on v3. Preserve those regression baselines and continue
with the remaining legacy/older-expiry/fault cases, snapshot/seed ownership and retained
RTC admission. The witness releases its freeze before completion; it is not an
ongoing ownership lease or complete media/capture activity proof. Do not repeat
the initial long build from scratch or ask again before these disposable tests.
The v4 research extension now retains a bounded empty-registry frozen lease,
but its first native test exposed application-script injection during that
lease. The experiment has been changed to revoke and thaw before script
dispatch. Its rebuilt native harness passes 50 checks in each default and
strict-site-isolation configuration, including main/child-frame injection;
the baseline and HTTPS regressions pass 40 and 26 checks. At v4,
queued/in-flight scripts and other media/capture categories were still
unqualified, so this was not production RTC admission.
The v5 research extension now tracks explicit Qt application scripts until
their engine callback, including no-callback child-frame requests. It refuses
a lease when one is pending and revokes/refuses an admission-racing request.
Scoped native tests pass 59 checks in each default and strict-isolation mode,
with 40/26-check baseline/HTTPS regressions passing. Other renderer-side
scripts, missing acknowledgements, all relevant media/capture categories and
Qt Quick ownership integration remain unqualified. Production stays disabled.
The later HTTPS edge fixture passed 33 checks on v5: browser responses clamped
a far-future expiry, rejected expired and invalid-attribute setters, and the
two accepted canonical records copied exactly to a fresh child. The 59/59 RTC
and 40-check baseline regressions still pass. This does not qualify legacy
provenance, opaque partition keys, older-expiry import, network-service failure
or production snapshot/seed ownership.
The v6 disposable native extension substituted `kUnknown` source provenance
and an 800-day expiry only in a safe synthetic snapshot's final in-memory
candidate. Both refused the whole copy before touching the child, and a separate
fresh child still copied the real 14-record source exactly. The 46-check native
baseline, 33-check HTTPS and 59/59-check RTC regressions passed. This proves
preflight ordering for fabricated candidates, not real legacy-profile import.
At v6, real legacy/opaque/fault coverage and production ownership remained open.
The v7 disposable native extension added a real opaque-partition cookie to the
off-the-record source store. Its 15-record native source snapshot refused the
whole copy before child writes; an independent existing-source copy to a second
fresh child refused too. The rebuilt probe passed 49 baseline, 33 HTTPS and
59/59 RTC checks with zero failures. This qualifies that specific opaque-key
refusal path, not opaque-key import or arbitrary browser-created opaque keys.
Real legacy/older-expiry import, network-service fault atomicity, source-T0 and
production child ownership remain open. See the v7 digests in `source-lock.json`.
The v8 disposable native fixture induced a real second-setter rejection after
one accepted child write. Its callback now reports `MutationRejected` and an
unknown child count (`-1`), rather than misleadingly claiming an empty child.
A diagnostic copy found one record in that failed child; the original source
still copied all 14 records to a separate fresh child. The 53-check native
baseline, 33-check HTTPS and 59/59-check RTC regressions passed. Discard the
failed child; this does not prove network-service loss, timeout atomicity, or
production source/child ownership. See the v8 digests in `source-lock.json`.
Pinned Chromium source inspection also shows that direct injection of a
`kUnknown` legacy cookie through the live Qt setter hits `CHECK_IS_TEST()` on
Linux, and that old overlong expiries are accepted from storage but not by the
ordinary sanitized setter. Continue with a controlled backing-store-load
fixture; do not crash the disposable network service merely to manufacture a
legacy record or call an in-memory substitution real import evidence.
A separate `probe-legacy` fixture now passes three fresh native runs of 16
checks each. In each of two disposable persistent Qt profiles, an ordinary
two-cookie source first reloads and copies exactly to a fresh child. After the
control exits, the runner modifies only fixed fields in that closed backing
store, then reopens it through Qt. Stored unknown-source provenance and an
unexpired 600-day creation-to-expiry span each trigger whole-list refusal with
zero child writes, despite a safe sibling. The runner issues no SQLite
`SELECT` or row export; no user profile is touched. See the digests and three retained
disposable paths in `source-lock.json`. This qualifies only disk-loaded
refusal, not historical-profile migration, legacy import, lossless older-expiry
copy, production ownership, or a Ferric capability flag.
Pinned Chromium source shows a possible but unqualified next step:
`CookieMonster::SetAllCookiesAsync` inserts full canonical records into its
persistent store without the ordinary setter, but its deprecated implementation
deletes the entire destination jar and Qt's Mojo `CookieManager` does not expose
it. Do not call it on a live profile. The next disposable prototype must
implement an engine-side bounded seed-if-empty operation with exact child
ownership/writer exclusion, then prove read-back and failure cleanup. See the
source-boundary analysis in `engine-capability-spike.md`.
The separate `qt-6.11.2-seed-if-empty-probe.patch` now tests that research
path in the disposable engine only. It has a bounded browser-only Mojo entry,
an empty-child check on the CookieMonster sequence, and a Qt exact-readback
probe using fresh persistent children. The rebuilt engine and probe passed
three fresh `probe-seed` runs of 40 checks each, with zero failures: both
disk-loaded edge cases seeded exactly in process and remained engine-readable
after normal-close restart. The 53-check cookie baseline, 33-check HTTPS and
59/59-check RTC regressions also passed. See `source-lock.json` and the
capability-spike note for exact fingerprints and retained disposable paths.
This is not an exclusive writer lease or production backend approval.
The restarted-child stage confirms two native canonical records and continuing
whole-list refusal for the edge record. Qt `cookieAdded` name signals were not
observed on restart, even after registering before page creation, and are
recorded only as a diagnostic. The restart witness is not an exact
cross-process canonical-member comparison.
Pinned SQLite backing-store source batches cookie additions; seed callback and
immediate exact read-back are in-memory evidence, not a durable commit. The
restarted-child stage covers normal-close persistence only, not crash
durability or a qualified flush/failure contract.
The pinned `FlushStore` completion is untyped and fires after a commit attempt
even when the underlying SQLite transaction failed; it is not a production
durability certificate by itself.
The seed fixture also tests a separate nonempty disposable child before either
source backing-store mutation. The engine seed entry refused it in all three
runs, and a subsequent ordinary copy found only its one original marker.
This does not establish an exclusive writer lease.
The separate `qt-6.11.2-t0-race-probe.patch` overlay then passed three fresh
7-check runs: after a complete-list callback captured two synthetic source
cookies, a native setter added a third before seeding; exact child read-back
and independent copies found source three versus child two. The 40/53/33/59/59
seed/cookie/HTTPS/RTC regressions also passed on this extended engine. This is
only a forced post-callback ordering witness, not a source-owner generation
lease or a test of unforced concurrent network/script/service-worker writers.
Its patch, binary and fixture digests are separate in `source-lock.json`.
The separate `qt-6.11.2-unpublished-child-probe.patch` overlay tested a fresh
persistent child with no application page. `cookieStore()` alone did not bind
its CookieManager and the seed returned `Unavailable`; a research-only hook
that initialized the default storage partition/browser-process CookieManager
without a page then passed three fresh 46-check runs. Both disk-loaded edge
cases seeded exactly before page creation, retained two native records after
page creation and normal-close restart, and the 40/53/33/59/59 native
regressions passed. This narrows the child-writer surface but does not prove an
exclusive CookieMonster writer lease, absence of other browser-process/native
writers, source-owner generation, crash durability or fault cleanup. The
shipping adapter remains unchanged and production capabilities stay disabled.
Pinned writer-path inspection confirms that the experiment's CookieManager
seed-busy bit is not an exclusive child lease: general CookieManager setters,
RestrictedCookieManager setters, and HTTP `Set-Cookie` responses enter the
shared CookieStore/CookieMonster through different routes. Any production
writer protocol must interlock there or prove the unpublished fresh child has
no such writers by construction and native stress evidence; see the spike note.
For RTC admission, the source-pinned media dispatcher has two distinct
in-flight gaps: it dequeues permission-handled requests before stream-start
counts rise, and preselected-device desktop capture bypasses that pending
queue entirely. A production absence check must interlock both paths as well
as renderer-side connection creation; neither an empty queue nor zero active
counts is a Clear certificate.
The 2026-10-04 continuation authored a disposable source-pinned RTC overlay
that starts one bounded pending-media token before either dispatcher branch,
retains it through permission and stream startup until active Qt counters are
visible, and invalidates a concurrent experimental freeze before routing a new
capture. Pending-permission, fake-audio-start, stopped-stream and permission-
denial fixtures were authored, along with a test-only invalid-ID witness
through the preselected-desktop dispatcher fast path. A separate follow-on
overlay and fixture now exercise one real synthetic-desktop route: page-side
legacy `getUserMedia` requests `screen:-3:0`, Chromium's fake desktop capturer
uses a bounded synthetic frame generator, and the fixture awaits capture start,
frame delivery, active-lease refusal and stop cleanup. It does not consult an
OS screen capturer. The native and strict-site-isolation RTC runs each passed
once, including permission, fake-audio, synthetic-frame, active-refusal,
stop-cleanup and counter-boundary cases. These focused results do not qualify
all renderer/media activity or production Clear; VB-033 remains blocked. See
the native capability-spike and VB-033 evidence.
The 2026-10-04 continuation also authored an isolated Qt Quick bridge overlay
for the exact `WebEngineView` owner: QML can request/check/release the retained
diagnostic lease, and the authored QML fixture tests empty admission, script-
dispatch invalidation and data-only peer refusal on that same view. The
overlay and fixture passed one focused run and close only the prototype's
source-level Widgets-versus-Qt-Quick API mismatch, not the production activity
gate. Production capability remains disabled and VB-033 remains blocked.
Source and runtime review made the acquire method single-flight and
revalidates exact adapter, hidden visibility and Frozen lifecycle state when
the asynchronous callback returns; the fixture now expects overlapping
acquisition refusal. Updated patch/fixture digests are in `source-lock.json`.
This hardening is built and tested in the focused prototype, but it does not
change any capability status.
The RTC registry fixture was then extended with a camera-only
`getUserMedia({video:true})` route against Chromium's synthetic fake device.
It holds video permission pending, requires pending/active lease refusal,
checks a delivered track frame, and awaits cleanup after stop; it does not
access an OS camera. This latest fixture was authored after the recorded
default/OOP runs and remains unbuilt/unrun under the verification deferral.
The source lock records separate latest and last-exercised fixture hashes.
No RTC or production capability status changed.
The exact Qt Quick owner fixture was also extended to request the synthetic
camera on its `WebEngineView`, require dispatch-time invalidation and a
delivered frame, refuse an empty lease while that track is live, then stop it
and await a fresh empty lease before repeating the data-only peer case. This
latest QML fixture is also unbuilt/unrun; the earlier one-run QML result remains
bound to its old fixture hash. The test grants only the fake video permission
and never opens an OS camera. Production capability remains disabled.
The baseline alone does not unblock production-dependent slices. The whole-product final campaign
remains separate. Start with
[`engine-capability-spike.md`](../../docs/testing/versioned-browsing/engine-capability-spike.md)
and [`tools/versioning-engine-spike`](../../tools/versioning-engine-spike/README.md).
The earlier 2026-10-01 current-build adapter recheck found a native GUI
blocker: the default-cache offscreen startup survived, but the uncached
variant crashed in Qt QML GC. The clean baseline survived uncached startup.
Temporary font-binding and profile-Loader ablations did not cure the crash
and were reverted. See the current-build section of
[`final-campaign-2026-09-29.md`](../../docs/testing/versioned-browsing/final-campaign-2026-09-29.md);
neither the cached smoke alone nor a GC environment override counted as GUI
acceptance.
The subsequent disposable local-QML-entry comparison narrowed the uncached
reproduction to the embedded BrowserScripts import in the local Surface, but
removing the unused production Surface import and rebuilding still crashed.
That test change and the temporary entry loader were reverted; this is not a
root-cause fix or an adapter pass.
The follow-on comparison removed all three unused BrowserScripts imports from
the runtime Chrome, Requests and Surface layers. Three uncached bounded
startups survived and the two-variant `cargo xtask test adapter` gate passed
without a GC override. A library-style script pragma ablation had failed and
was reverted. This is an observed startup repair, not proof of the Qt GC root
cause or qualification of any cookie/WebRTC capability. The diagnostic loader
was removed and `cargo xtask test adapter` passed again on that final source
state. `cargo xtask check --locked --offline` also completed without a QV4 or
QML-cache override, while the public cookie fixture remained unqualified; see
the updated final-campaign evidence.
VB-022 is verified for structural commit/list on the current Qt build. A real
disposable normal-profile Qt commit survived forced process restart, and the
restarted process accepted `commit-list`. The panel's asynchronous list-arrival
QML suite passes 15/15 after correcting its hidden test host; the live native
accessibility probe found the exact saved ID under `Saved commits` in the
target process when Qt's process-local accessibility switch was enabled. Its
four helper regressions pass. See `VB-022.md`; this does not qualify page
archives, cookies, live retention, or branch restore.
VB-025's compact native keyboard continuation found a real 627 × 694 layout
failure: the restore confirmation was outside the dialog. The restore-review
column is now constrained to its parent width, and focus follows list →
details → safe Cancel review → list transitions. The focused QML panel suite
passes 17/17. After rebuilding, a disposable placeholder commit was inspected
and restored by compositor keyboard events addressed to the exact Ferric
window: Return on the list, Return on details preview, Tab from Cancel to the
now-visible confirmation, and Return opened a second same-process native
window while the source stayed open. The confirmation's AT-SPI bounds were
inside the 627 × 694 dialog, and a later list result refocused the list. This
is structural keyboard/UI evidence, not multi-page safe-GET, sidecar,
screen-reader, cookie, WebRTC or current-window replacement qualification;
VB-024 remains in progress. The complete `cargo xtask check --locked
--offline` passed after these changes, with 153 QML passes and 5 gated skips;
the native Wayland archive input and public cookie identity checks remain
unqualified. See `VB-024.md` and `VB-025.md`.
VB-025 is now verified for its explicit keyboard-only UI acceptance. A current-
source regression rejects an ambiguous short commit prefix in both the panel
and application parser; the focused panel suite passed 18/18, pure planner
diff and application redaction tests passed, and complete offline workspace
check, strict application Clippy and formatting passed. This does not promote
VB-024, production archive/cookie/WebRTC or an independent screen-reader
audit; see `VB-025.md`.
VB-024's subsequent disposable two-page native safe-GET continuation found
two reducer tabs and two native views but only one accessible tab in the
restore child's strip. The tab strip now flushes Qt's batched `ListView`
layout after reconciliation and binds count-dependent presentation to the
source model. Focused chrome QML passed 29/29; the rebuilt child exposed two
page tabs while the source stayed mapped and only the selected loopback
document was requested on restore. The background tab still presents
`about:blank`; safe deferred URL activation, committed sidecars and the
active-download/dirty-page lifecycle remain unqualified. The complete
`cargo xtask check --locked --offline` then passed (154 QML passes, 5 gated
skips, workspace Rust suites), while archive-input and public cookie probes
remained unqualified. Do not promote
VB-024 from this partial native evidence. See `VB-024.md`.
The next VB-024 continuation repaired the deferred safe-GET path: inactive
replayable URLs are retained in a bounded, exact-owner/document/lineage
application map, and one ordinary navigation starts only when the exact
still-blank tab is activated. A pre-mutation capacity check refuses a
restore that cannot retain all its deferred URLs. The native tab strip now
offers accessible Press and keyboard activation, and labels a pending tab
“Restored page — select to load” without exposing its URL. In the disposable
two-page native journey, the selected page alone loaded on restore; Pressing
the unloaded tab requested `/final` exactly once, and a second Press made no
request. Focused application and QML regressions passed (30 chrome QML).
The complete workspace check passed on the capacity-guarded source (155 QML
passes, 5 gated skips). Sidecar, download/shutdown and current-window replacement gates
remain open; do not promote VB-024 from this partial evidence. See `VB-024.md`.
The native archive-input probe now passes in a disposable Weston headless
Wayland compositor with a fake seat, Pixman renderer and kiosk shell. The
xtask runner starts and tears down that compositor when available and falls
back to the active Wayland session otherwise. This resolves the probe's
active-desktop focus skip for the isolated test path; it does not qualify
Ferric's integrated page capture or committed-sidecar restore and does not
enable production archive capture. See `VB-045.md` and the final campaign.
The complete `cargo xtask check --locked --offline` passed with this probe
executed, 155 QML passes, 5 gated skips and workspace Rust suites. The
disposable compositor/socket/lock were absent afterward.
The native archive probe subsequently confirmed that immediate and started
page-save cancellations reached `DownloadCancelled` without an archive, while
`downloadFinished` was absent within bounded 15-second waits on Qt 6.11.2.
An immediate cancellation reported `isFinished` despite never completing;
the started cancellation had not yet set `isFinished` at its state signal.
Ferric now retires only the exact correlated cancelled/interrupted capture
from that terminal state, without waiting for the finish callback, and keeps
the 90-second deadline fallback. `cargo xtask test archive` passed. The
integrated cancellation and remaining VB-057 cases are still unqualified;
production archive capture remains disabled. See `VB-045.md`.
The complete `cargo xtask check --locked --offline` also passed (157 QML
passes, 5 gated skips, native probes and workspace Rust suites), together
with formatting, diff, pack validation and checksum checks.
The VB-045 terminal cleanup is now a shared production QML/JS helper with a
focused regression: only the current exact cancelled/interrupted request is
retired, duplicate/stale/completed calls are ignored, and late callbacks keep
their path/ID quarantine. ChromeInteractions passed 31/31; the complete
`cargo xtask check --locked --offline` passed with the regression included.
This tests Ferric's cleanup logic but not its full live capture journey;
the archive production gate remains disabled. See `VB-045.md`.
VB-043's file-only worker now has a deterministic concurrency witness: it
pauses mid-scan on a 4 MiB staged archive while the separate SQLite metadata
worker completes a query, then resumes and installs the exact object. The
test-only pause does not alter production scheduling. Focused and full storage
tests passed (163/163), as did strict storage Clippy. This proves worker
separation, not latency under device saturation or the integrated Ferric
capture journey; VB-043 remains in progress. See `VB-043.md`.
VB-045 also has a coordinator/worker failure regression: after real consent,
reservation and stage creation in a disposable profile, deletion of the exact
stage file makes object installation fail. Cancelling that operation releases
its reservation and leaves zero visible commits. The focused application
test passed. This is a missing-stage/native-loss analogue, not disk-full or
an integrated Qt-view-loss qualification; production capture stays disabled.
The complete `cargo xtask check --locked --offline` passed with this fault
regression included, together with formatting and diff checks.
Strict affected-crate Clippy passed after a scoped length annotation on the
end-to-end fault test.
VB-044/045 capture-abort handling now quarantines the stage path and both
request IDs before Qt cancellation, stops an original in-flight save when a
duplicate/mismatched request arrives, and clears the capture before deadline
or operation-retirement cancellation. Locally identifiable ownerless saves
also fail closed. The focused QML cleanup suite passed 32/32, including
stale and throwing cancellation cases; `cargo xtask check --locked --offline`
passed. Real integrated duplicate/ownerless/deadline journeys remain open,
and production archive capture remains disabled. See `VB-044.md` and
`VB-045.md`.
VB-044's exact page-save request admission now lives in one shared production
QML callback function, exercised by the disposable native Qt archive probe.
The real `downloadRequested` signal produced one exact ID/view/path/MHTML
admission and a completed nonempty archive; a second save refused by the mock
Rust admission was cancelled, retired by ID and path, and left no file. The
focused QML route regression passed 34/34, and `cargo xtask test archive`
passed including the durable MHTML adapter follow-on. The complete `cargo
xtask check --locked --offline` passed afterward (169 QML passes, five gated
skips, native probes and workspace Rust suites). This still uses a mock
Rust admission rather than a full Ferric normal-profile capture, so VB-044
and VB-057 remain open with production capture disabled; see `VB-044.md`.
VB-044's native probe now holds a real ordinary Qt download request, admits
an exact production-routed page save, and accepts the ordinary request while
that save remains pending; both finish independently. Immediate refusal
before this overlap leaves no file, and the default `cargo xtask test archive`
passes. A retained opt-in reverse-order diagnostic,
`FERRIC_ARCHIVE_PROBE_REFUSAL_AFTER_CONCURRENT=1 cargo xtask test archive`,
repeatedly fails: after the overlap, immediate refusal of the next save is
followed by a later save stuck `DownloadInProgress` despite bytes on disk.
Omitting refusal lets the later save finish. Deferring Qt cancellation lets
it finish but briefly creates a refused archive path, so that experiment was
reverted; do not use it as a production fix without bounded native cleanup.
Keep VB-044/VB-057 open and production disabled. See `VB-044.md`.
The complete `cargo xtask check --locked --offline` passed after the overlap
probe was added, including default native probes, QML checks and workspace
Rust tests. The reverse-order diagnostic is opt-in and still fails.
Further native instrumentation on Qt 6.11.2 showed the refused request
immediately reports `DownloadCancelled`/`isFinished=true`, yet emits no
`downloadFinished` during a bounded wait before the next save. After the next
save starts, the refused ID later emits `downloadFinished` as
`DownloadCompleted` while the new save hangs. Its file remains absent.
Pause-before-cancel behaved the same and was reverted. Do not use a simple
finish-signal wait as the release workaround; see `VB-044.md`.
An opt-in disposable Qt 6.11.2 experiment replaced `WebEngineView.save()`
with `triggerWebAction(SavePage)` and set the private path/MHTML format in
the pending request callback. Its accepted save and ordinary download
completed, but reverse-order refusal still hung the subsequent save. Letting
Qt default-reject the unaccepted pending request instead of calling
`cancel()` also hung it. Both experimental variants were reverted; the
production route, passing default probe, and disabled release gate are
unchanged. See `VB-044.md`.
The 2026-10-03 opt-in reverse-order probe still fails on the current Qt
build: the refused request has no file and reports cancelled/finished, but
the later save writes 82,058 bytes without reaching a terminal callback.
Qt's public download-request contract permits page-save auto-start and
recommends deleting unneeded request objects after the signal returns. A
disposable variant performed that exact deferred deletion and verified the
refused object was gone; the later save still hung with no refused file.
The variant was removed. Default `cargo xtask test archive` still passes,
including the durable MHTML adapter follow-on. No public-API workaround is
qualified; keep VB-044/VB-057 open and production capture disabled. See
`VB-044.md`.
VB-044's runtime work loop now retires an old native save when Rust presents
a different exact pending capture, rather than leaving the old QML slot held
until its 90-second deadline. It quarantines old path/ID before Qt cancel,
discards the old probe, cancels only the old Rust operation, then admits the
new one. A full quarantine refuses replacement without dropping the old
capture. The focused helper regression failed before the fix and the final
ChromeInteractions suite passed 36/36; `cargo xtask test engine` passed 425
Rust tests and the QML suite after the production scheduler edit. This does
not solve the separate Qt reverse-order refusal hang or qualify a live
replacement capture; see `VB-044.md`.
The VB-045/056 delayed-publication regression now holds a successful storage
callback past the operation deadline and callback window after SQLite has
committed the manifest. The application stays `OutcomeUnknown`, releases its
reservation without deleting the durable commit, and refuses to claim
completion when the late success arrives. Focused application and strict
Clippy checks passed; crash/fsync fault injection and integrated Qt capture
remain unqualified. The normal `commit --pages` route still rejects sensitive
commits and the archive UI action is disabled, so neither was bypassed to
count this lower-level test as release evidence. See `VB-045.md` and
`VB-056.md`.
The complete `cargo xtask check --locked --offline` passed with this
delayed-publication regression included; formatting and diff checks passed.
For VB-024, nested restore owners now share the entry window's 32-descendant
budget. A hidden, shutdown-approved restore parent is retained until its
descendants close, while discovery and shutdown/cancel requests still traverse
through it. The entry shutdown path counts the retained tree and excludes its
hosts from the separate live-registry pass, avoiding duplicate close/cancel
dispatch. The new QML regressions passed in the 2026-10-02 workspace check.
The asynchronous page-state shutdown probes
in entry, secondary and popup windows now reject changed native view identity,
navigation epoch, URL, loading or active-tab set before accepting a clean
result. Their final close boundary rechecks late activity even after page-state
override or storage retry. Page-state override is now bound to the view and
load context shown with the prompt; a changed context restarts the probe. The
focused QML state fixture passed; this does not freeze
same-document editable state. Native active-download/dirty-page shutdown,
multi-page safe-GET/sidecar journeys and real profile-wide download callback
delivery remain unqualified; see `VB-024.md`.
The profile-owner download route now considers its own tabs, same-profile
secondary windows, popups, restore descendants and their popups. It is bounded
and refuses unknown or ambiguous ownership instead of selecting the first
retained child or silently applying a callback to the entry window. The
secondary forwarded route reuses its existing handler without adding a shared
profile signal receiver. These QML fixtures passed. A disposable
two-view native Qt profile-download fixture is also staged as
`cargo xtask test versioning-profile-download` (and in `xtask check`), but not
executed successfully in the 2026-10-02 workspace check. It cannot by itself qualify
Ferric's multi-window routing. Pinned Qt 6.11.2 QML
download source also lacks a null guard in `view()` despite documentation of
viewless downloads, so actual shared-profile and viewless callback behavior
requires native qualification before a full VB-024 acceptance claim.
The later native profile-download fixture now exercises Ferric's production
restore-window owner and profile routing functions with real shared-profile
Qt request/finish callbacks: source and restored host each own their exact
view, while ambiguous and ID-only forged claims refuse. `cargo xtask test
versioning-profile-download` passed. This supersedes the earlier unexecuted
fixture note, but is not a full Ferric UI/durable-ledger, attachment,
viewless-getter or active-download shutdown qualification. See `VB-024.md`.
The complete `cargo xtask check --locked --offline` passed with the expanded
fixture (155 QML passes, 5 gated skips, workspace Rust suites).
VB-024's native restored-child dirty-page shutdown refusal exposed a real
root-gate cancellation bug: both windows stayed mapped, but the source later
displayed its shutdown timeout. A child owner now calls the root host's abort
only while application shutdown is active. The focused owner QML suite passed
15/15; after rebuilding, a saved editor child refused `:quit`, both windows
remained open without a timeout after 12 seconds, and a second `:quit` opened
a fresh root confirmation. This covers one dirty-page cancellation path, not
active-download shutdown, same-document mutation, sidecars, viewless
downloads or current-window replacement. See `VB-024.md`.
The complete `cargo xtask check --locked --offline` passed on this source
(156 QML passes, 5 gated skips, native synthetic probes and workspace Rust suites).
The VB-024 shared-profile Qt fixture now includes an 8 MiB slow loopback
attachment: the restored view's real request was partially received, cancelled,
and finished with `DownloadCancelled` on the same object and restored host.
The focused `cargo xtask test versioning-profile-download` passed. A disposable
Ferric entry window showed the active-download `:quit` warning while the
transfer's TCP connection was established; “Keep browser open” preserved the
process and a second quit opened fresh confirmation. This does not qualify a
restored-child active-download shutdown, Ferric durable ledger, viewless
callback or cancel-and-quit path. See `VB-024.md`.
The complete `cargo xtask check --locked --offline` passed with the streamed
download probe included; the production gate remains closed.
VB-024 now has integrated restored-child active-download shutdown evidence:
a one-use local editor arm made an existing saved commit open in a child and
start a slow attachment. Native `:quit` reached that child's activity prompt.
Keeping it open preserved both windows and allowed a fresh quit. Cancelling
active work advanced to the child's page-state warning. The source's timeout
prompt had previously remained after child approval; the root owner now
rechecks shutdown when child discovery/removal changes, even after its timer
stops. Focused owner QML passed 16/16; after rebuilding, child page-state
approval led to clean process exit without a root timeout interaction. The
synthetic download row was `cancelled` with zero bytes. Full ledger semantics,
partial-byte accounting, viewless callbacks, sidecars and replacement remain
open. See `VB-024.md`.
The complete `cargo xtask check --locked --offline` passed on this source
(157 QML passes, 5 gated skips, native synthetic probes and workspace Rust suites).
VB-024's accepted restored-child download continuation found and repaired
three linked defects: no live received-byte ledger updates, the one-shot Qt
request guard refusing `cancel()` after `accept()`, and terminal Qt byte
counter reset erasing recorded progress. Progress updates are bounded to
roughly one per second; an unfinished accepted request permits one later
cancel; storage retains the row's byte high-water mark until a new offer.
The focused storage regression passed. A rebuilt native child transfer had
851,968 bytes in progress, then whole-browser cancel-and-quit left a durable
`cancelled` row with 6,389,760 bytes, no completion timestamp and no final or
staging file. Page-state approval still closed the browser normally. The
broader VB-024 sidecar, viewless-callback, late-race and replacement gates
remain open; see `VB-024.md` for the intervening negative runs.
The committed-sidecar native journey is not yet admissible: a disposable
`commit --pages --editable` request returned `E_ENGINE`, and the documented
`versioning.archive_capture` gate is primitive-supported but production-disabled
pending capture-orchestration qualification. Do not use UI injection to bypass
that gate or count an uncommitted sidecar as restore evidence.
One VB-024 viewless-download probe tried deleting a restored WebEngineView
during an active disposable transfer. Same-turn cancellation left the view
wrapper live at callback; after deferred deletion, `qmltestrunner` crashed in
Qt QML signal evaluation. No OOM was reported. The experimental change was
removed and the original three-request native probe passes again. Synthetic
null-view/request-identity routing is covered, but a truly viewless native
terminal callback is still unqualified; see `VB-024.md`.
VB-024's top-document shutdown probe previously returned `clean` for a
plaintext-only editable draft, an embedded child document, and a changed
input inside an open shadow root. It now reports `unknown` for embedded
documents/objects, any contenteditable attribute, open shadow roots,
custom-element hosts and a traversal beyond 4,096 elements. The native Qt
page-state fixture first reproduced the false-clean result, then passed 30
checks with five loopback-fixture skips; the broader offscreen QML suite
passed 174 with six gated skips. `cargo xtask test
versioning-geometry` passed the live frame/Rust sidecar admission. Ordinary
clean/dirty controls retain their distinct outcomes. This narrows shutdown
loss risk but does not prove arbitrary page-script or closed-shadow state;
the VB-024 late-race and viewless/sidecar gates remain open.
VB-055's optional media seek now rechecks the exact allowed source fingerprint
immediately before changing `currentTime`. A synthetic source-swap QML case
passed with no seek or playback call; the focused page-state suite passed 27
checks with four loopback-fixture skips and the native geometry admission probe
passed. This is helper evidence only, not committed-sidecar qualification; the
unkeyed fingerprint privacy decision and VB-057 gate remain open.
The subsequent VB-055 threat review confirmed that the persisted unkeyed
media-URL digest is dictionary-testable and cross-capture linkable for a
sidecar-only reader. The inspected versioning path has no secret/key lifecycle,
so no capture-nonce "key" or same-store ad hoc secret was added. A core
regression now keeps the hint out of Debug, safe summary and declined-media
restore requests; only an explicitly opted-in helper request receives it.
The focused test, strict core Clippy and formatting passed. The product/security
risk decision and integrated media journey remain open; see `VB-055.md`.
VB-048/055 current-source focused recheck passed all 18 core page-sidecar
tests, 27 offscreen QML page-state checks with four live-fixture skips, and
native Rust sidecar admission (`cargo xtask test versioning-geometry`),
including four editable values and four categorical exclusions. The preceding
complete workspace gate also included these helpers. This corrects the
earlier ledger's stale “unrun” wording; integrated committed Ferric capture,
live media seek, the unkeyed fingerprint privacy decision and VB-057 remain
open. See `VB-048.md` and `VB-055.md`.
VB-048 now fails closed when editable-control `id`, `name`, `autocomplete`
or `aria-label` metadata exceeds its 256-character inspection bound (or is
unreadable/non-string). Before the fix, four synthetic overlong-metadata
controls had their values captured under explicit editable consent; after it,
the QML regression reports zero values and four fixed `sensitive-control`
exclusions. The current-source offscreen page-state suite passed 28 checks
with four live-fixture skips, and native Rust sidecar admission passed.
The full workspace gate has not yet been rerun on this change. This narrows a
privacy edge but does not lift the integrated capture or production gate.
VB-049 now validates each renderer frame callback's raw string and bounded
arrays before merging it into the aggregate sidecar. A synthetic callback
first admitted 65 candidates against the 64-item per-frame limit; after the
fix, that and an over-256-KiB response fail closed. The local fixture now
serves a ten-document same-origin chain; the collector admits exactly nine
frames, reports the depth-nine limit exclusion and partial coverage, and
Rust admits the bounded sidecar. The current-source Qt offscreen suite passed
29 checks with five fixture skips, and `cargo xtask test versioning-geometry`
passed native frame-tree/Rust admission. The full
workspace gate has not yet been repeated on this change. Integrated
owner/document capture intervals and VB-057 remain open; see `VB-049.md`.
VB-042's production consent host now refreshes from Rust after either result
of an exact page decision. A failed acceptance can have consumed/cancelled its
preview; the old QML host left that stale dialog visible. It also forwarded
an old dialog signal after a replacement disclosure. Two focused regressions
failed before the repair; exact-operation checking and scoped retirement now
pass 9/9 offscreen consent-host checks, including refused decline. The broader
offscreen QML suite passed 173 checks with six environment-gated skips. This is
QML presentation/callback evidence, not a live Ferric capture or gate lift;
see `VB-042.md`.
VB-050 manifest publication validation now refuses a supported archive object
without an archive interval and any sidecar object without a sidecar interval;
monotonic pairing remains mandatory. The focused regression first accepted
those false-provenance writes, then passed after the fix. Older stored
manifests remain readable under the compatibility validator, but cannot be
newly written without the missing facts. Object-backed test fixtures now
carry explicit intervals. Storage versioning passed 72/72, strict storage
Clippy and the complete `cargo xtask check --locked --offline` passed,
including native probes and workspace Rust suites. Integrated collector/
archive failure journeys and VB-057 remain open. See `VB-050.md` and
`VB-093.md`.
VB-050 additionally rejects `complete-for-request` page fidelity when a
stored sidecar has no category evidence, at both application publication and
manifest validation; older stored records remain readable. The final-state
application and storage suites passed 161/161 and 162/162 tests and strict
Clippy passed for both crates. The complete `cargo xtask check --locked
--offline` also passed, with the affected suites rerun after the final
compatibility adjustment. A numeric
count gap alone is intentionally not treated as incomplete: the collector
uses a minimum requested count of one on empty pages. Integrated native
collector/archive failure journeys and VB-057 remain open.
VB-050 now also rechecks the active native view and URL after the asynchronous
mutation probe, before submitting saved terminal-download facts to Rust. A
changed or destroyed view/route becomes `document-changed` and cancels
publication. The focused QML ChromeInteraction suite passed 33/33 checks,
including switched-view, changed-URL and destroyed-view regressions. Integrated
native collector/archive failure journeys remain open. The complete `cargo
xtask check --locked --offline` passed afterward (168 QML passes, five
environment-gated skips, native probes, workspace Rust tests).
VB-050 now has three durable adapter callback integration cases through an
actual disposable profile, one-use native consent, staging, object worker and
manifest publication. Collector failure with archive success commits
archive-complete/page-unavailable; valid sidecar collection with interrupted
archive commits nothing; an injected sidecar-install callback failure with
archive success commits archive-only. The focused suite passed 3/3, full Qt
crate suite 423/423 and strict Qt Clippy passed. Synthetic archive bytes,
download facts and callback fault injection do not qualify a real Qt save or
disk failure. Keep VB-050 and VB-057 open. See `VB-050.md`.
VB-050's disposable Weston archive probe now exports the real QtWebEngine
MHTML file only after its complete native save/view/isolation run passes.
`cargo xtask test archive` then feeds those bytes through the one-use durable
Ferric capture adapter and object worker before the disposable directory is
removed. The native probe and ignored adapter fixture passed; the manifest
records the Qt file size and archive-complete/page-unavailable fidelity. The
strict Qt Clippy check and complete `cargo xtask check --locked --offline`
also passed (168 QML passes, five environment-gated skips, native probes,
workspace Rust suites). The download facts and page URL are still synthetic
across this process boundary,
so this does not qualify a live Ferric save callback, disk fault, or VB-057.
Production capture remains disabled; see `VB-050.md`.
VB-050's page-capture adapter now supports an explicitly consented archive
for a current HTTP(S) page whose durable navigation descriptor is an unsafe
POST/non-replayable placeholder. The earlier collector required a stored
replayable URL and failed before issuing a challenge. Capture-local route
correlation now uses the trusted current reducer tab under the exact
owner/document/revision checks; a stored replayable URL must agree when one
exists. A regression failed before the fix, then committed an archive-only
manifest after malformed collector output while retaining the placeholder,
refusing a wrong observed route, and recording archive-complete/page-
unavailable fidelity. Focused callback tests passed 4/4 (one fixture-gated
native test ignored), Qt check and strict Clippy passed. The complete
`cargo xtask check --locked --offline` also passed. This is
synthetic adapter evidence, separate from the native POST save below and not
VB-057 release qualification; see
`VB-050.md`.
VB-050's disposable native archive probe now posts a same-origin form to a
loopback result page, verifies the POST body and exact native save admission,
and exports the completed POST-result MHTML after its archive-isolation checks.
The durable adapter fixture publishes those Qt bytes under a one-use capture
while retaining a non-replayable navigation placeholder; archive fidelity is
complete and page-sidecar fidelity unavailable. `cargo xtask test archive`
passed with both native and durable legs. The earlier synthetic adapter URL
and native callback metadata are still separate, and a save attempted after
the probe's other refusal/concurrency scenarios did not finish. Do not enable
production capture or claim VB-057; see `VB-050.md`.
The live page-state fixture also submits its loopback form as POST, qualifies
the current result route, collects a one-frame sidecar, and passes that native
JSON through Rust's exact origin/nonce/fingerprint admission. `cargo xtask
test versioning-geometry` and strict xtask Clippy passed. This independently
proves collector behavior on a POST result, but not one integrated
QML-to-Rust/native-save callback chain; keep VB-050 and VB-057 open.
VB-051/052's focused archive-view tests now pass for exact materialization,
late owner replacement, corrupt/missing-object refusal and temporary-file
cleanup. The disposable Weston native archive probe also passes after adding
exact-permit switching and revocation: neither a previously rendered stale
file nor a now-revoked file reloads, and no loopback traffic occurs. This
probe now also loads the actual production archive QML component on demand:
its exact MHTML load and successful `about:blank` acknowledgement pass while
the temporary file still exists, and the ordinary source profile continues
loading afterward. The production chrome's cleanup callback, timeout/retry,
and full hostile-resource matrix remain unqualified; keep archive viewing
disabled.
The production component's hostile external-link native route now passes:
the clicked link is rejected and sends zero loopback requests. Immediate
blanking after that rejected navigation timed out on this Qt build, retaining
the file; explicit retry reached blank acknowledgement. The bounded native
result was `hostile_link_rejected=1;hostile_blank_retry=1;network_hits=0`.
This qualifies safe retry behavior, not reliable first-attempt blanking or the
production chrome's storage-cleanup callback. Protocol/download and direct
CSS/image/frame interception remain open; keep archive viewing disabled.
The production component's native hostile fixture now also clicks visible
download and unregistered-protocol links. Both are rejected at its view;
three disposable-Weston runs passed with zero download requests and zero
loopback traffic. The probe now requires these refusals and scopes profile
inspection to the current Loader instance. Qt did not emit a download
request for this MHTML link, so the cancellation callback itself remains
unqualified, as do frame/CSS/image profile interception and the integrated
blank-to-storage-cleanup handoff. Keep archive viewing disabled.
An embedded `data:` download control now completes as a real Qt download in
ordinary source HTML. Its exact link survives Qt MHTML save, but two native
archive runs observe no view or download callback for the same click, no file
and no loopback traffic; a later known link still reaches the view. This
narrows the gap to MHTML-sandbox suppression on this build. It does not
exercise Ferric's download-cancellation callback or integrated cleanup, so
keep the offline release gate disabled.
The production runtime chrome's existing archive overlay and blank-to-cleanup
callback now reside in `FerricVersioningArchiveHost.qml`. A disposable native
probe exercised that exact host with a fixture UI bridge: a valid MHTML was
shown without premature cleanup; successful blank led to exactly one cleanup
request and runtime schedule, viewer deactivation, file retention until the
application worker, and zero loopback traffic. The focused boundary test, all
415 engine Rust tests and 159 QML checks passed (5 gated skips). This does
not join the host to a real `BrowserUiRust`/storage completion in one Ferric
window. Keep archive viewing disabled pending that journey and the remaining
hostile-resource/engine-capability gates.
VB-051/VB-052 now also test the production archive host's cleanup-refusal
retry through the native disposable Weston probe. A first exact bridge
refusal after blank retains the viewer and staged file without scheduling
storage work; a second close schedules one cleanup, deactivates the viewer,
and causes no network request. `cargo xtask test archive` passed, including
the durable MHTML adapter follow-on. This is a synthetic bridge refusal,
not real `BrowserUiRust`/storage completion in one Ferric window; keep
archive viewing disabled. See `VB-051.md` and `VB-052.md`.
VB-051's archive interceptor now refuses reuse on a second live OTR profile;
a test-first native assertion failed before this repair and passed afterward.
Its exact-file URL and one-use file/blank permits are consumed together under
one lock, preventing stale requests from spending a newer permit generation.
`cargo xtask test archive` passed in disposable Weston with the durable
adapter follow-on. This does not cancel already admitted requests or qualify
the remaining full-app isolation paths; keep archive viewing disabled.
VB-052's application cleanup scheduler now avoids submitting a second removal
for an archive operation with an existing cleanup ticket. The durable archive
fixture seeds a pending ticket, checks repeated profile-teardown calls, then
performs one real removal. The focused test, all 161 application tests, and strict Clippy
passed. This does not establish blank-before-cleanup ordering across a live
profile lifetime change; that integrated lifecycle remains open.
VB-052's production archive host now captures the exact active operation ID
before load and passes it after blank; BrowserUi refuses a malformed or
non-current ID and clears the identity only after scheduling exact cleanup.
The native host fixture failed before this wiring and then passed in
disposable Weston, including its retry and durable adapter legs. The focused
Rust identity guard test passed. This prevents delayed callback retargeting,
but does not qualify blank-before-cleanup ordering on profile teardown or
the full real BrowserUi/storage journey; archive viewing stays disabled.
VB-051 now refuses `BrowserUi` profile configuration both before and after
asynchronous profile-open work if an archive request, viewer URL, active
operation, or cleanup is still owned by the current profile. A focused Rust
state test and strict Qt-crate Clippy passed. This closes a direct state-swap route
around the viewer, but native shutdown and application-level owner-rebind
blank-before-cleanup ordering remain unqualified; do not enable viewing.
VB-052 now marks an archive file handed to Qt as potentially in use. App
owner-rebind and whole-profile automatic cleanup retain it until the view's
explicit blank-acknowledged close; a test-first durable fixture failed on
premature scheduling before repair and then passed. A storage restart test
confirms a retained private MHTML file is quarantined after the old owner
stops. All 161 application and 171 storage tests passed. This work follows
the storage-engine safety invariant that reader-owned files are not deleted
before release, while recovery can reclaim orphaned private bytes. A
test-first regression then reproduced live GC moving an active viewer file.
Viewer operations now use a separate private `views` root: live GC leaves it
alone, and restart recovery validates and quarantines it after the old owner
stops. An in-flight copy/GC race and an unsafe substituted viewer leaf are
covered by regressions; all 174 storage tests pass. The release gate stays
off pending the real native/application/storage shutdown journey and the
remaining VB-051 interception/integration qualification; see `VB-052.md`.
VB-052's normal owning-window shutdown now waits for archive preparation,
presented URL blanking, and exact cleanup before flushing metadata and
requesting Rust shutdown. It asks the production archive host to blank the
view, retries after a blank timeout, and retains the existing force/cancel
choice if the bounded shutdown deadline expires. The Rust phase-state test
and runtime/host boundary assertions pass. A failed cleanup after confirmed
blank can leave private bytes for restart quarantine. This narrows the
shutdown-ordering gap but is not yet a native real-window/real-storage
end-to-end journey; the release gate remains off. See `VB-052.md`.
The complete `cargo xtask check --locked --offline` passed after the
shutdown-ordering repair, including the workspace Rust suites and native
synthetic probes.
VB-026/VB-052/VB-056 now include a killed-child viewer-recovery test. The
disposable child stops after moving a retained private MHTML viewer operation
into quarantine and syncing the destination and source directories. Reopen
preserves the exact bytes in that first batch with no duplicate move. The
focused test, full 175-test storage suite, strict storage Clippy and
formatting passed. This follows the storage-engine process-death recovery
invariant; it is not power-loss or mid-sync evidence, nor the real-window
blank-to-storage cleanup qualification. See `VB-026.md`, `VB-052.md` and
`VB-056.md`.
VB-051/VB-052 now qualify the successful real-window archive shutdown route
on this Qt runtime. `cargo xtask test archive` first produces a verified
MHTML under disposable Weston, publishes it through the durable adapter,
then reopens that disposable profile in the shipping `Main.qml` window with
its actual `BrowserUi` and storage worker. The test waits for exact archive
presentation, observes blank acknowledgement, forbids removing the viewer
file while its URL remains presented, and requires exact-file removal before
shutdown approval. Its first run exposed a deferred open after shutdown
blanking; `openAuthorizedArchive` now refuses to re-arm while closing, and
the native production-view probe checks that guard. Archive-load reporting
also ignores initial blank loads. The strengthened native/adapter/window
campaign passed. Keep `versioning.archive_offline` disabled pending remaining
CSS/image/frame/form interception, unobserved download-cancellation behavior,
real storage-refusal/retry, and broader archive-versus-live profile/UI checks.
See `VB-051.md` and `VB-052.md`.
The complete `cargo xtask check --locked --offline` then passed with 176 QML
passes, seven environment-gated skips, native probes and workspace Rust
suites. Strict Qt-crate Clippy, formatting and whitespace checks passed after
three mechanical test-harness lint fixes.
VB-051/VB-052 now also exercise real storage-worker cleanup refusal in the
shipping window. After exact archive presentation, a test makes only the
disposable viewer leaf unsafe (`0644`). Normal shutdown blanks the view; the
worker refuses removal and retains the file. Earlier runs observed a retry
and sometimes successful test-side permission repair, but repetition found
the worker can exhaust its immediate retries before the test restores mode.
That race does not qualify deterministic successful retry. The revised test
keeps the leaf unsafe through shutdown and requires blank acknowledgement,
safe retention and shutdown approval. The runner builds one lib-test artifact
and runs each Qt GUI journey in its own process.
This narrows, but does not close, VB-051/VB-052: CSS/image/frame/form and
download-cancellation callbacks remain unobserved on this MHTML runtime,
and active-view profile change and broader archive-versus-live UI checks
remain open. Keep the offline release gate disabled.
The same real-window journeys now attempt the production `configure_profile`
path with a different ephemeral profile while the archive is presented.
`BrowserUi` refuses the switch and retains the same archive URL, active
host and exact viewer file. This qualifies
active-view profile-switch refusal only. Post-close profile replacement and
other owner transitions still need qualification. Strict affected-crate
Clippy passed after this native check was added.
VB-051/VB-052 post-close profile qualification now has a fourth native Rust
leg. The real window refuses profile configuration while the archive is
presented, then the production host blanks it and the real worker removes
the exact private file. Once host and cleanup are settled, the same
`BrowserUi` accepts a fresh ephemeral profile; the old durable commit is
unavailable there. The first run revealed periodic metadata polling trying
to restart a durable worker in that transient profile and overwriting its
ready status. The UI now skips durable-worker restart and library queries
for transient profiles. `cargo xtask test archive` passes the native probe
and all four Rust legs after the fix. This is an adapter transition in a
real window, not complete visible multi-window profile qualification.
The unsafe-file route now deterministically proves blank-first retention
through shutdown; immediate successful cleanup retry remains unqualified.
The archive release gate stays disabled.
The complete `cargo xtask check --locked --offline` then passed on this source
with 176 QML passes, seven environment-gated skips, native probes, four
archive Rust legs and the workspace suites. Strict Qt/xtask Clippy, workspace
formatting and whitespace checks passed.
VB-026/VB-052/VB-056 now join the real-window unsafe-leaf shutdown to
first-owner restart quarantine. The native test keeps the mode-`0644` viewer
file through blank acknowledgement and approved shutdown, stops the old GUI
owner, restores only that disposable leaf to private mode, and reopens the
same durable profile. Recovery removes the old loadable viewer path and
preserves exactly one quarantined MHTML copy with bytes matching the verified
Qt fixture. The expanded `cargo xtask test archive` passed its native probe
and four Rust legs. This is normal-close recovery after a controlled
permission repair, not unrepaired-unsafe-leaf, power-loss, mid-sync or
killed-GUI evidence. Broad fault qualification and the archive release gate
remain open. Strict affected-crate Clippy and workspace formatting pass on
the expanded test source.
For VB-026, the orphan-object quarantine scan now refuses files with multiple
hard links, consistent with corrupt-reference quarantine. A synthetic outside
hard-link regression passed in the workspace Rust suite. This does not qualify
the broader crash/fault campaign; see
`VB-026.md` and VB-091.
VB-026 corrupt-reference quarantine now prevalidates the entire bounded
digest set before any move and treats a wholly missing shard as an absent
object. Focused recovery tests passed 5/5, including invalid-after-valid
input, an over-limit set and a missing-shard no-op. The broader crash/fault
campaign is still open.
The complete `cargo xtask check --locked --offline`, strict storage-crate
clippy and workspace formatting checks passed after this repair.
VB-026 corrupt-reference recovery now also prevalidates every present source
before creating a batch or moving an object, so a late unsafe hard link cannot
strand an earlier reference. Unix identity/ownership/link checks repeat after
preflight and before each move. A separate post-first-move injected-I/O hook
returns an error with one private partial batch; restart retry quarantines the
other three entries and a further retry is a no-op. Eight focused recovery
tests pass, including a fault-hook replacement of a private inode; all 56
storage versioning tests, strict storage Clippy and formatting pass. A same-UID
check-to-rename race and later I/O partial batches remain outside this proof;
the broader VB-091 campaign is still required. See `VB-026.md`.
The next VB-026/VB-056 process-kill fixture covered the original five object-
install checkpoints and exposed a recoverable post-object-link state that the orphan sweep previously refused: the staging
`object.tmp` and unpublished object have the same private two-link inode.
Recovery now accepts only that exact unreferenced pair and quarantines both
links. A referenced pair or an external hard link still refuses before any
move. The actual killed-child regression and negative cases pass in the 59/59
storage versioning suite; strict storage Clippy and formatting pass. This is
not a disk-full/fsync-failure, all-publication-step, same-UID-race, or
integrated Qt capture qualification. Keep VB-026/VB-056 in progress and
continue VB-091. The complete `cargo xtask check --locked --offline` passed
afterward. See `VB-026.md` and `VB-056.md`.
VB-026 restart orphan quarantine now rechecks each scanned source's Unix
identity, owner, type and link count before creating its batch and immediately
before its move. A fault hook that replaced the second orphan with an outside
hard link after the first durable move reproduced an unsafe move before the
fix; it now refuses with the outside sentinel intact and one recoverable
partial batch. The storage versioning suite passed 60/60, strict storage
Clippy passed, and the complete `cargo xtask check --locked --offline` passed
(166 QML passes, five gated skips, native probes and workspace Rust suites).
This narrows the check-to-move gap, not same-UID adversarial
atomicity or the full VB-091 campaign; keep VB-026 in progress. See `VB-026.md`.
The adjacent VB-026 staging recovery case now inventories at most the four
module-owned leaves per operation and rechecks their exact names, Unix
identity, owner, type and link count before batch creation and each stage
move. A post-first-move replacement of `object.tmp` by an outside hard link
failed its refusal test before the fix and now refuses without moving that
link. Focused recovery passed 11/11, storage versioning passed 61/61, strict
storage Clippy and the complete `cargo xtask check --locked --offline` passed
on this later source state. Same-UID races and VB-091 remain open. See `VB-026.md`.
VB-026 restart recovery now rejects malformed or over-limit referenced digest
sets before any directory scan or quarantine move. A new invalid-reference
test first reproduced an unsafe successful orphan move; after the preflight
check, that case and the over-limit case leave orphan/staging paths intact
with no batch. Recovery passed 13/13, storage versioning 63/63 and strict
storage Clippy passed. This does not close the broader fault campaign; see
`VB-026.md`. The complete `cargo xtask check --locked --offline` passed
afterward, including 166 QML passes, native probes and workspace Rust suites.
VB-026 next reproduced an unsafe parent-shard swap: after preflight, a
private object shard was moved outside the profile and its old path replaced
with a symlink. Corrupt-reference quarantine returned an error but had already
moved the original outside file. The fixed path rechecks the profile/object/
shard directory chain before each move. A restart-orphan variant swaps the
remaining shard after one durable move and now leaves that object outside
intact while retaining the earlier partial batch. Storage versioning passed
65/65 and strict storage Clippy passed. Same-UID final-check/rename races and
VB-091 remain open; see `VB-026.md`. The complete
`cargo xtask check --locked --offline` passed afterward on this source state.
VB-026 restart recovery previously changed terminal capture outcomes (`failed`,
`cancelled`, `partial`, and `outcome-unknown`) to `interrupted`, losing their
durable recovery metadata. A regression reproduced this on the current source.
Recovery now marks only nonterminal operations interrupted; all six terminal
states retain their metadata, and a terminal-only restart writes no false
journal event. Focused storage versioning passed 66/66, strict storage Clippy
and workspace formatting passed. Complete `cargo xtask check --locked --offline`
passed on rerun (167 QML passes, five gated skips, native probes and workspace
Rust suites). Its first run failed in a separate context-commit application
test; that test passed 20 focused repetitions and the 159-test application
suite. The later same-profile publication investigation reproduced its two
causes: competing first-open schema setup and a second owner's restart
recovery interrupting a live capture. Migration now re-reads under its write
lock, same-process database opens serialize by canonical path, and subsequent
owners share one recovery result. Fresh/upgrade stale-preflight, concurrent
open, shared-failure and live-capture regressions pass; the formerly flaky
application suite passed 20 consecutive runs. This does not close VB-026 or
VB-091; see `VB-022.md` and `VB-026.md`. The complete
`cargo xtask check --locked --offline` passed on the repaired source, including
167 QML passes, five gated skips, native probes and workspace Rust suites.
The shared recovery registry is process-local; cross-process exclusion still
depends on the existing profile lock.
VB-026 now has actual child-process kills at all five commit-deletion
transaction checkpoints. Four pre-commit kills preserve the commit and its
object on reopen; the post-commit kill leaves a deferred orphan that restart
GC quarantines. The focused regression and storage versioning suite passed
68/68, and strict storage Clippy passed. This is instrumented process-death
atomicity, not filesystem power-loss or complete VB-091 fault qualification;
keep VB-026 in progress. The complete `cargo xtask check --locked --offline`
passed afterward, including native probes and workspace Rust suites. See
`VB-026.md`.
VB-026/VB-056 also now kill a disposable child immediately after the first
synced restart-quarantine move. Reopening finishes the three remaining moves
into a second private batch, accounts for all four entries, and makes a later
retry a no-op. The focused test, storage versioning suite (70/70), and strict
storage Clippy passed. This does not qualify disk-full, fsync failure or
power-loss recovery. The complete `cargo xtask check --locked --offline`
passed afterward, including native probes and workspace Rust suites. See
`VB-026.md` and `VB-056.md`.
VB-026 now injects `ENOSPC` after the first 16 KiB temporary-object copy
chunk of a 32 KiB staged capture. The installer returns an error without an
object receipt, removes the partial temporary file, and restart recovery
quarantines the staging operation with no orphan object. The focused test,
storage suite (164/164), and strict storage Clippy passed. This is a test-only
partial-copy fault, not actual disk exhaustion, fsync failure or complete
VB-091 qualification; keep VB-026 in progress. The complete `cargo xtask
check --locked --offline` later passed with this regression. See `VB-026.md`.
VB-026 now also injects `EIO` at both directory-sync boundaries after an
immutable object is linked and its temporary name removed. Both paths
withhold the receipt, and restart quarantine recovers the unacknowledged
orphan and staging operation. The focused test, full storage suite (165/165)
and strict storage Clippy passed. This is boundary injection, not a real
mid-`fsync` failure or power-loss guarantee. See `VB-026.md`.
The complete `cargo xtask check --locked --offline` passed afterward,
including default native probes and workspace Rust tests.
The VB-026 killed-child object-install matrix now also covers the two
post-link directory-sync boundaries: seven checkpoints total. In both new
kills, restart sees no temporary name and quarantines the unacknowledged
object link plus staging operation. The focused child test, storage suite
(165/165), and strict storage Clippy passed. This is process-kill evidence,
not power-loss durability. See `VB-026.md`.
VB-026/VB-043/VB-056 now test the metadata-worker handoff after an
immutable object is installed: a manifest with the correct digest but wrong
size is rejected, explicit abort leaves no visible commit, and restart
quarantines the private orphan while preserving the durable `failed` state.
The focused regression, complete storage suite (166/166), and strict storage
Clippy passed. This is deterministic validation failure, not native Qt
capture, worker process death or power-loss qualification; see `VB-026.md`.
VB-026/VB-056 restart quarantine now injects `EIO` before four directory-sync
boundaries: new-batch parent, first destination, first source, and final
quarantine parent. Each failure withholds success; reopen/retry accounts for
all three unpublished entries while preserving a referenced object and then
becomes idempotent. The focused regression, storage versioning suite (77/77),
full storage suite (167/167), strict storage Clippy and formatting passed.
This models syscall-boundary returns, not an actual mid-`fsync` failure or
power loss. VB-026/VB-056/VB-091 remain open; see `VB-026.md` and `VB-056.md`.
VB-026/VB-056 now have killed-child coverage for a referenced corrupt object
after its private quarantine move/directory sync but before the SQLite GC
transaction commits, and after commit. The pre-commit kill exposed a false
`missing` state on retry despite bytes in quarantine. Recovery now scans
private UUID batches once, bounded to 100,000 entries, and recognizes only
validated `corrupt-<digest>` files; it records the commit's object as
`quarantined`, never restores it, and refuses a symlink spoof. Both kill
points, the spoof regression, 170 storage tests, strict storage Clippy and
formatting passed. This is process-death evidence, not power loss or a real
mid-`fsync` error; VB-026/VB-056/VB-091 remain open.
The complete `cargo xtask check --locked --offline` also passed on this
source, including 176 QML passes, seven gated skips, native probes and
workspace Rust suites.
The first complete `cargo xtask check --locked --offline` run found a stale
VB-024 `BrowserScripts.VERSION` test assertion (expected 10; current script
is 11). The focused contract test passed after correcting that assertion,
and the complete check then passed with 174 QML passes, six gated skips,
native probes and workspace Rust suites. This is test-contract alignment,
not completion of VB-024's remaining native journeys; see `VB-024.md`.
VB-022/VB-056 now have actual child-process kills at six checkpoints across
object-backed SQLite publication. Every pre-commit kill reopens with no
commit, object row/link or capture journal entry and retains the staging
reservation; a post-commit kill reopens with all rows and a complete operation
together. The immutable object remains intact. The focused test, storage
versioning suite (72/72), and strict storage Clippy passed. This is
process-death transaction atomicity, not power-loss/fsync-failure durability
or browser-facing acknowledgment qualification. The complete `cargo xtask
check --locked --offline` passed afterward, including native probes and
workspace Rust suites. See `VB-022.md` and `VB-056.md`.
Archive-view materialization no longer opens its target with truncation before
checking the descriptor's identity and link count. A synthetic outside-hard-link
regression passed in the workspace Rust suite; the broader fault/race campaign remains open.
VB-027 policy-preview cancellation and VB-026 deletion-preview cancellation
now require the originating window, like their confirmation paths. A wrong-window
deletion confirmation no longer consumes the rightful owner's preview. Focused
application regressions passed in the workspace Rust suite.
VB-027 now has one disposable native policy-confirmation journey on the
current Qt 6.11.2 build. A structural commit captured
`content.javascript=false`; after the current value changed to true,
`restore <commit> --policies` displayed the exact allowlisted diff without
mutating config. An immediate AT-SPI Press on the visible production action
applied false as a temporary override and opened a same-profile restore child
while the source window stayed live. An earlier nested Weston kiosk-shell
restart crashed in `kiosk-shell.so`; the standard-shell retest succeeded.
Stale-revision/cancel native cases, ordinary restore without policies and the
VB-024 dependency remain open as broader restore qualification. See `VB-027.md`.
An additional VB-027 application regression now applies an ordinary
pristine-window structural restore whose manifest captures the opposite
`content.javascript` value. The current value and configuration revision
remain unchanged, and a durable notification permission grant in the profile
store is unchanged as a record. The focused case and all 131 application
versioning tests, strict application Clippy, formatting, and full offline
workspace check passed. VB-027's bounded policy transaction is verified;
a separate native ordinary-restore journey remains unqualified. See `VB-027.md`.
VB-028's first disposable native context-create/enter/save journey found that
the Qt save handler compared a core profile display label with the context's
stable profile name; it rejected the default `Default`/`default` pair. Save and
same-context-window lookup now use the exact current profile name and source
core `ProfileId`. A second disposable profile saved membership and durably
published a one-window context-scoped structural commit with a bounded
interval and no layout. Native `window-new` then exposed a duplicate ID:
two independent core sources both allocated `windowid-2`. Qt bootstrap now
uses a process-shared monotonic ID source, and the QML registry refreshes its
secondary identity after bootstrap with a collision guard. Focused core/runtime
tests and a new disposable process confirmed distinct live IDs (`windowid-6`
and `windowid-16`); both adapter startup variants and 20 focused context tests
pass after the change. At that point the primary application runtime still
contained only its own window, and the primary IPC owner rejected a command
addressed to the second live ID. This exposed the separate routing and
cross-window capture-coordination requirements. See `VB-028.md`.
VB-028 now has bounded exact-secondary IPC handoff for typed, explicitly
addressed command/action requests. A fresh disposable offscreen two-window
process returned real secondary `windowid-16`/`profileid-15` command contexts,
new tab IDs for both command and action, and accepted secondary context
create/enter/save; an absent target was rejected. A stale membership refused
an addressed context commit until resaved; the subsequent commit became a
durable `scope=context` row with exactly one manifest window. The primary retains a
different `profileid-5`. This closes the direct-command routing gap only.
The two application projections are still separate, so one context-save and
one context commit cannot yet aggregate both windows. Owner-level window and
operation queries are not federated. Keep VB-028 in progress; build a bounded
cross-owner capture coordinator and native two-window commit witness before
promoting it. Strict Qt Clippy, formatting, and the complete `cargo xtask check
--locked --offline` pass after this routing change. See `VB-028.md`.
The application-level cross-owner capture core now accepts separate mutable
normal-profile `BrowserApplication` owners, preflights saved membership against
exact live window/tab state and matching durable profile UUIDs, and assembles
one context manifest before identity binding. A two-owner disposable Rust test
reached a durable row with two manifest windows; a different-profile peer was
excluded without binding. The 31 focused capture tests, all 156 application
tests, and strict application Clippy pass. This does **not** yet connect the
separate Qt `BrowserUi` owners:
native `context-save` is still local, peer reducer effects need exact-owner
delivery, and no native two-window context commit has been witnessed. Continue
the Qt capture handoff and negative native cases; keep VB-028 in progress.
An additional non-serializable owner-issued `ContextWindowCaptureCandidate`
seam now projects a single exact window without binding it. The source
compares candidate context ID, profile UUID, member ordinal, live tab order,
and logical IDs with its current saved registry before assembling a manifest;
the originating owner separately rechecks its native instance and reducer
revision before binding. Application tests cover two-owner candidate assembly,
a missing peer recorded as closed, and stale peer binding refusal after a tab
change. All 156 application tests and strict Clippy pass. At this point the
Qt bridge still needed a bounded opaque-handle handoff, per-owner effect
checkpointing, a federated context-save command path, and a native two-window
publication witness. Do not interpret this core seam as an integrated Qt capture.
The Qt adapter now stages owner-issued candidates in a bounded process-local
Rust mailbox, keyed by random opaque handle, typed window ID and exact live
owner token. QML sees only the current saved context UUID and a handle, never
the structural payload. Only the originating `BrowserUi` can consume the
handle and apply binding effects through the revision-aware checkpoint bridge;
the mailbox expires entries and caps them at 256. `cargo check -p
ferric-browser-engine-qt --locked --offline -q`, two focused mailbox tests,
all 157 application tests, and strict Qt Clippy pass. This is **not** yet a working native
multi-window context command: source-side handle assembly, federated
`context-save`, durable publication coordination and a native two-window
witness remain. Keep VB-028 in progress.
VB-028's source-side Rust plan and interactive Qt coordinator are now wired:
the source snapshots all unique opaque handles against its exact saved
membership, retains the immutable manifest/context, and publishes only after
the originating owners bind and the mailbox verifies every one-use receipt.
The QML command path obtains saved member IDs from Rust, preflights exact
window IDs/owner tokens, and delegates the capture/bind/finish sequence; QML
never sees structural payload. Application tests passed 157/157, the focused
two-owner test repeated 10/10 after correcting aggregate interval start to
the earliest owner capture, three Qt mailbox tests passed, a new focused QML
coordinator test passed, and Qt check/strict Clippy passed. This remains
**partial**: `context-save` is still local, addressed IPC context commits are
not federated, and no native two-window durable witness exists. Federate
membership save before claiming this route works end-to-end; keep VB-028 in
progress. See `VB-028.md`.
VB-028 now has a native, same-process **two-window** witness. `context-save`
uses a second bounded Rust-only mailbox: each live normal-profile window
stages a safe membership candidate, rechecks its profile UUID/native owner,
reducer revision, context and tabs, and emits a one-use confirmation. The
source saves only after all exact owners confirm. Interactive and addressed
IPC structural context commits use the already-built capture coordinator.
In a fresh disposable offscreen profile, `windowid-6` and `windowid-16`
were saved into one context record and one durable `scope=context` commit
contained two manifest windows, ordinals 0/1, bounded per-window intervals,
and no serialized runtime window IDs or layout. After both windows gained
tabs, a stale-membership commit wrote no row; re-saving then produced a
second durable two-window commit with page counts 2 and 3. Restart preserved
both rows and refused old runtime membership without refresh. The 157
application tests, 23 focused Qt context tests, seven QML coordinator cases,
Qt check, and strict Qt Clippy passed. A full `cargo xtask check` first found
the pre-existing IPC module exceeded its 1,000-line limit after the new
branch; the branch was moved to a focused module and the complete
`cargo xtask check --locked --offline` rerun passed. VB-028 is verified for
normal-profile structural context capture, not for cookies/live retention or
the later structural release milestone. See `VB-028.md`.

The public Qt backend is still unsupported for strict cookies/live-call absence.
Chromium canonical snapshot/seed is a promising **different backend**, not an
impossibility finding: its synthetic 14-record exact copy now passes natively,
using the complete-list callback and full canonical records inside the engine.
Host-only scope survives in canonical domain text.
WebRTC observation alone never proves absence or closes the freeze race. Merge
requires separate writer-control qualification. Do not enable production flags
or mark slices verified merely because the isolated probe compiles/passes.

VB-051/VB-052 now have a fifth disposable native archive leg that uses a
non-default test-only feature to make the actual object worker refuse one
exact cleanup after a verified MHTML is presented in the shipping window.
The normal blank-to-cleanup route then retries, removes the exact private
file and approves shutdown. `cargo xtask test archive` passed all five
separate-process Rust/Qt legs and the native isolation probe. This supersedes
the earlier open item for a controlled one-shot worker-error retry, but does
not qualify recovery from an OS filesystem fault or a permanently unsafe
leaf. The unsafe-leaf retention/reopen-quarantine leg remains independent.
Do not enable archive viewing: VB-051 resource interception and download
cancellation remain unobserved on saved MHTML, and broader profile/UI
qualification is still open. See `VB-051.md` and `VB-052.md`.
The complete `cargo xtask check --locked --offline` then passed on this
source (176 QML passes, seven gated skips, five archive Rust legs, native
probes and workspace tests); strict affected-crate Clippy passed.
VB-026/VB-056 now account for `quarantined` as well as `ready` object-index
bytes at capture reservation and publication. A focused near-2 GiB test with
a retained commit link failed before the change and passed afterward; all
176 storage tests, strict storage Clippy and formatting passed. This prevents
one form of quota reuse while corrupt bytes are retained. It is not a hard
physical disk cap: unindexed orphan/quarantine batches, native save overshoot
and SQLite/WAL overhead still require the wider VB-091 campaign. No retained
data is deleted to free budget. See `VB-026.md` and `VB-056.md`.
The complete `cargo xtask check --locked --offline` passed on this source,
including QML, native and workspace suites.
VB-026/VB-056 now include a Linux-only disposable-child `RLIMIT_FSIZE`
object-copy regression. The child sees actual kernel `EFBIG`, no object is
installed, and parent-side restart recovery quarantines the exact staged
32 KiB bytes once. The full storage suite passed 178/178, with strict
storage Clippy and formatting. This is not `ENOSPC`, a mid-`fsync` failure,
power loss or native Qt save qualification; VB-091 remains open. The fixture
uses a child Bash resource limit because repository policy forbids unsafe
Rust even in tests. See `VB-026.md` and `VB-056.md`.
The complete `cargo xtask check --locked --offline` subsequently passed,
including QML, native probes and workspace tests.
VB-026/VB-056 now also test a real SQLite `max_page_count` refusal during
structural publication after reservation. A bounded 128-page manifest forces
`SQLITE_FULL`/`DiskFull`; reopen sees no commit or capture journal entry,
then an increased limit lets the same operation publish exactly once. The
full storage suite passed 179/179 with strict storage Clippy and formatting.
This is not OS disk exhaustion, a failed sync, power loss or object-backed
native capture qualification. See `VB-026.md` and `VB-056.md`.
The complete `cargo xtask check --locked --offline` passed afterward,
including QML, native probes and workspace tests.

VB-026/VB-056 now also cover a real SQLite page-limit refusal while publishing
an already-installed archive object. No commit, object index or capture journal
leaks out of the failed transaction; first-owner restart quarantines the exact
orphan bytes and marks the operation interrupted. The focused regression,
180/180 storage tests and strict storage Clippy passed. This is metadata
allocation failure, not native Qt save, physical ENOSPC, failed `fsync`, or
power loss; the release gate remains disabled. See `VB-026.md` and `VB-056.md`.
The complete `cargo xtask check --locked --offline` passed afterward,
including QML, native probes and workspace tests.

VB-024 now routes profile-wide download completions by retained exact request
identity in both root and secondary windows, without re-reading the Qt view
getter after a tab might have been destroyed. Unknown secondary completions do
not update a ledger, and archive completion refuses an unavailable captured
view URL. Focused QML owner/routing suites passed 16/16 and 6/6, including a
synthetic throwing getter. This does not qualify initial viewless requests or
an actual post-destruction native callback; the prior native attempt crashed.
Qt documents Frozen as suspending most, not all, HTML tasks, so it is not
treated as a same-document mutation lease. See `VB-024.md`; production flags
remain disabled.

The complete `cargo xtask check --locked --offline` passed after its first
run exposed one static Rust contract assertion for the old secondary helper
signature; the focused contract test passed after correction. This does not
turn the synthetic no-getter QML case into native view-destruction evidence.
Strict engine-Qt Clippy and formatting passed too.

The subsequent VB-024/VB-045 review found that request identity alone was
too weak for archive completion: the accepted request may outlive its page.
Completion now also requires the captured view to remain the active view and
its URL to be readable, without touching Qt's late download-view getter.
Focused ChromeInteractions QML passed 37/37, including wrong-request,
closed/switched-view and throwing captured-view fixtures. Native callback
after actual view destruction is still unqualified, and production stays off.
Pinned Qt 6.11.2 source shows the profile-level download signal is the only
QML route for download actions; a request without a QML page has a null
adapter client, which the documented `download.view` getter dereferences.
There is no public per-view signal to replace that origin lookup. Do not
fallback to the active tab; keep true viewless shared-profile callbacks
unqualified pending a null-safe/public-origin runtime contract.
The complete `cargo xtask check --locked --offline` passed afterward (177 QML
passes, native probes and workspace tests); formatting and pack validation
passed too.

VB-026/VB-056 now include retained unpublished quarantine in page-capture
admission. The metadata worker uses the trusted normal-profile state root to
inventory bounded, private UUID quarantine batches and counts stage, viewer
and orphan-object file lengths alongside indexed objects, manifests and live
reservations. Unknown or unsafe entries refuse the capture without deletion.
A worker regression retains a sparse abandoned capture and confirms the next
maximum reservation fails with no durable operation; recovery regressions
count stage/object bytes and reject an object-name symlink without following
it. All 183 storage tests, strict storage/application Clippy, formatting and
the complete `cargo xtask check --locked --offline` passed. This is not an
exact physical quota: live staging/viewer files, native overshoot, SQLite/WAL and
external allocations remain outside admission accounting. VB-026/VB-056 and
the full VB-091 campaign remain in progress; see their testing records.

VB-026/VB-056 page-capture admission now also accounts for live immutable
objects left unindexed by a rejected publication before restart recovery.
The metadata worker loads at most 100,000 ready digest/size rows and scans a
bounded private SHA-256 tree, checking names, ownership, mode, type, link
count and size agreement. It adds only unindexed object lengths to the
existing reservation/quarantine calculation; unsafe entries refuse without
deleting bytes. A real-worker over-budget orphan regression and exact-budget
indexed-object non-double-count regression pass, as do scanner mismatch and
symlink refusals. All 187 storage tests, strict storage/application Clippy,
formatting and complete `cargo xtask check --locked --offline` passed. This
is still not an exact physical quota: live staging/viewer files, native
overshoot, SQLite/WAL and external allocations remain outside the inventory;
VB-026/VB-056/VB-091 remain open. See their testing records.

VB-026/VB-056 page-capture admission now inventories private live viewer
copies and unreserved capture staging as well as indexed objects, live
orphans and quarantine. View copies count in full and are never moved by
admission. Durable active operation IDs are parsed as typed `OperationId`s
and matched to their UUID staging directories: an abandoned/finished stage
counts in full, while an active stage contributes only observed bytes above
its existing reservation. Unsafe entries refuse without deletion. A worker
regression refuses retained-view and abandoned-stage bytes at the budget
edge; another refuses active-stage excess and admits the exact boundary once
the disposable test file returns to its reserved size. Symlink and arithmetic
scanner tests also pass. All 194 storage tests, strict storage/application
Clippy, formatting and complete `cargo xtask check --locked --offline` passed.
This is a point-in-time conservative admission check, not a hard filesystem
cap against subsequent native writes, SQLite/WAL, allocation overhead or
external files. VB-026/VB-056/VB-091 remain in progress; see their records.

VB-050/VB-053/VB-056 now include a synthetic archive-plus-sidecar callback
journey that installs both objects, publishes exactly one commit, and reads
back the exact committed sidecar for its logical page. The adapter rejects a
second collector result after the first result is handed to the object worker,
preventing a manifest-summary/installed-byte mismatch in that window. This
does not yet combine a real native collector and MHTML save with durable
publication or qualify native restore/failure handling. Production gates stay
closed. The focused regression and complete `cargo xtask check --locked
--offline` passed; see the three testing records.

VB-050 callback interval races now have durable tests for same-document
mutation (two objects retained, page fidelity partial), navigation after
sidecar collection (no mixed-document commit), and a collector result after
archive start (sidecar omitted, archive-only commit). The late-result guard
prevents reversed sidecar/archive intervals. The callback suite passed 8
active tests and strict Qt-crate Clippy passed; its 4 native fixtures remain
separately gated. This is still not one live native collector/save/publication
journey; see `VB-050.md`.

VB-050/VB-053 now have one continuous native success path through the
production window on a disposable loopback GET document. A real fixed-consent
collector result and real Qt MHTML save pass the one-use adapter, publish one
archive-plus-sidecar commit, survive profile reopen, and the exact committed
sidecar read re-qualifies against its route and disclosure challenge. The
journey exposed a missing `route_fingerprint` and bounded `max_ms` in the
production QML collector request; both are now supplied. Earlier statements
that all native collector/save/publication evidence was separate are
superseded for this one case. POST/editor/frame/mutation races, native failure
matrix, restore application and VB-057 release qualification remain open;
production gates stay closed. See `VB-050.md`, `VB-053.md` and `VB-056.md`.
The complete `cargo xtask check --locked --offline` passed on this source
state, including the continuous native journey and workspace Rust suites;
strict Qt-crate and xtask Clippy passed too.

VB-050 now additionally has a continuous native same-document mutation
journey through that production window. A synthetic loopback page changes
while the real collector and Qt MHTML save run. The persisted commit retains
archive and sidecar objects and intervals; after profile reopen, the exact
sidecar re-qualifies and the manifest records `observed-changed` with
`partial` page fidelity. `cargo xtask test archive` passed both static and
mutating live journeys; the complete `cargo xtask check --locked --offline`,
strict Qt-crate/xtask Clippy, formatting and pack validation passed too.
Native navigation and failure combinations,
POST/editor/frame capture, restore application and VB-057 remain open;
production gates stay closed. See `VB-050.md`.

VB-050's live production-window native journey now also covers a navigation
after the real sidecar is accepted and the MHTML save has been requested. The
replacement route must become the fully loaded active view; after profile
reopen, no mixed-document commit exists. The strengthened `cargo xtask test
archive` campaign passed this race with the stable and mutating journeys.
Strict Qt-crate/xtask Clippy, formatting, ledger and checksum validation
passed; the earlier complete project check predates this test addition.
Native collector/archive failure combinations, POST/editor/frame capture,
restore application and VB-057 remain open; production capture stays
disabled. See `VB-050.md`.

VB-050 now also has a live cancelled-archive failure journey. After a real
collector sidecar is accepted, a test-only C++ diagnostic verifies the exact
operation, Qt download ID and stage path, then invokes that save request's
public `cancel()` slot. QML and Rust capture requests both retire, and
profile reopen finds no commit. `cargo xtask test archive`, strict Qt-crate/
xtask Clippy and formatting passed this case alongside stable, mutating and
navigation journeys. Collector failure with a successful archive, disk fault,
POST/editor/frame capture, restore application
and VB-057 remain open; production capture stays disabled. See `VB-050.md`.

VB-050 now has the opposite live failure outcome too: a test-only native
diagnostic queues a bounded renderer task through the public WebEngineView
script API, making the real collector exceed the production QML two-second
deadline while the Qt MHTML save subsequently succeeds. The journey directly
observes the collector deadline; after profile reopen, exactly one commit
contains an archive interval and archive-complete fidelity, with no sidecar
interval and page fidelity unavailable. `cargo xtask test archive` passed the
full native archive campaign; complete `cargo xtask check --locked --offline`,
strict Qt-crate/xtask Clippy, formatting and pack validation passed too. Disk
fault, POST/editor/frame capture, restore application and VB-057 remain open;
production capture stays disabled. See
`VB-050.md`.

VB-050's live production-window campaign now includes an actual sidecar
object-worker refusal. A feature-gated, one-shot worker hook refuses only
the sidecar install after the real collector and Qt MHTML save succeed. The
test observes one consumed worker refusal; profile reopen finds one
archive-complete commit with no sidecar object, page-unavailable fidelity and
both capture intervals. `cargo xtask test archive` passed. This is not a real
OS disk fault. The complete `cargo xtask check --locked --offline`, strict
storage/Qt/xtask Clippy and formatting passed too. POST/editor/frame capture,
restore application and VB-057 remain open with production capture disabled.
See `VB-050.md`.

VB-050's live production-window campaign now also verifies one real OS
sidecar write denial. A bounded test-only pause allows mode 0500 on the exact
capture staging directory inside a disposable normal profile. The normal
worker path observes one `PermissionDenied` from the attempted sidecar file
creation; the test restores mode 0700 before UI completion handling. After
durable profile reopen, one archive-complete commit retains the real Qt MHTML
object and archive interval, but no sidecar object and unavailable page
fidelity; the attempted sidecar collection interval remains recorded.
`cargo xtask test archive`, strict feature-enabled storage/Qt/xtask Clippy,
formatting, pack and checksum validation passed on the final source state.
This is a specific filesystem permission failure, not general disk exhaustion
or full VB-057 qualification. Production capture remains disabled; see
`VB-050.md`.

VB-050 now has a continuous native POST-result production-window journey.
The real renderer submits a body-bearing form to a route that cannot be
replayed with GET; the fixture runner requires exactly one POST and no GET to
that result. This exposed and fixed a primary-view URL-binding feedback loop:
Qt's WebEngineView URL property loads with GET, so the view releases the
binding for a form navigation and rearms it only for a distinct later model
target. Real collector and Qt MHTML save then publish one durable commit;
profile reopen confirms both objects and intervals with a non-replayable URL
placeholder. The same journey proves a subsequent ordinary GET navigation
still works. `cargo xtask test archive` and complete `cargo xtask check
--locked --offline` passed. Editor/frame/media journeys, secondary-window
form navigation, arbitrary POST redirects, restore and VB-057 remain open;
production capture stays disabled. See `VB-050.md`.

VB-050 now also has continuous native frame and editor journeys. The frame
fixture publishes one archive-plus-sidecar commit whose stored sidecar
contains the top document and same-origin child plus one cross-origin
exclusion. The editor fixture explicitly opts into editable capture and
publishes one bounded synthetic draft in its committed sidecar without
logging private bytes. Both survive durable profile reopen with independent
intervals; all 15 `cargo xtask test archive` journeys pass. Arbitrary frame
changes, real-site editor sensitivity, media devices, secondary windows,
restore and VB-057 remain open. Production capture stays disabled; see
`VB-050.md`.

The live VB-050 campaign now also captures local media metadata. A real,
deterministic one-second loopback WAV reaches `loadedmetadata` in the
production primary view before fixed consent. The reopened commit has both
objects and a sidecar with one paused media-time entry; its source filename
is absent from sidecar bytes. The WAV fixture test and all 16 native archive
journeys pass; strict affected-crate Clippy and the complete offline project
check pass on this source state too. This does not qualify remote/live/
protected streams, device capture, secondary windows, restore or VB-057.
Production remains disabled;
see `VB-050.md`.

VB-050's separate secondary browser window now also preserves a form POST
result without GET replay. The child releases its URL binding when the main-
frame request has form data and rearms it for a distinct model target. A
continuous same-profile native journey submits the POST-only fixture, sees
the result remain loaded, then navigates normally. The fixture runner
requires two body-bearing POSTs across primary and child journeys, and all
17 native archive journeys pass. A follow-up run traced the child's stalled
normal close to undeclared pending-download fields, then found that an
approved close hid the dynamic child without destroying or unregistering it.
The child now initializes those fields and destroys itself only after an
approved close makes it invisible. The same native journey exercises close
confirmation, page-state handling and durable shutdown; it observes child
destruction before root shutdown without a test-only delete. This qualifies
one synthetic same-profile child close, not arbitrary descendants. Secondary-
window capture publication, arbitrary redirects, restore and VB-057 remain
open; production capture stays disabled. See `VB-050.md`.

Strict workspace Clippy and complete `cargo xtask check --locked --offline`
passed on this close-flow source state (177 QML passes, seven gated skips,
all 17 native archive journeys and workspace Rust suites).

The 17-journey archive campaign and complete `cargo xtask check --locked
--offline` now pass together on the secondary POST continuation (177 QML
passes, seven gated skips and workspace Rust suites). The per-journey outer
archive timeout is 180 seconds so the test's bounded internal phases can
report their own failures. A mutating-page rerun demonstrated a transient
status-text race after a durable commit; the test now observes capture/request
retirement and relies on its existing reopened-profile commit/object proof.
This is test-harness hardening, not additional production qualification.

VB-050 now also has two same-origin POST redirect journeys. A fixed 303 form
must produce one body-bearing POST followed by one GET result; a fixed 307
form must preserve the POST body at its result. The fixture runner checks all
four exact observations and rejects duplicates or GET replay. Both real
production-window journeys capture the result through the collector and Qt
MHTML save, then reopen one durable archive-plus-sidecar commit; the 307
result is a non-replayable placeholder. Later ordinary navigation still
rearms the URL binding. The focused fixture test and all 19 native archive
journeys pass. Arbitrary and cross-origin redirects, secondary-window capture
publication, restore and VB-057 remain open; production capture stays off.
The complete `cargo xtask check --locked --offline` also passed on this
19-journey source state (177 QML passes, seven gated skips, native probes and
workspace Rust suites), as did strict workspace Clippy.

VB-053 now has one real capture-to-destination geometry journey. A loopback
GET page's two nested scroll offsets are committed with its native sidecar,
then applied in a fresh browser window after durable profile reopen. All 20
native archive journeys pass. This exposed and fixed process-shared ID
preview consuming the live allocator, and premature restore startup before
the blank destination's load had settled. An exact provisional URL update
now retains its one-use sidecar intent; a different route still invalidates
it. Focused core/application tests pass. Broader changed-site and multi-page
native qualification and VB-057 remain open; production capture/viewing gates
remain disabled. The complete post-change `cargo xtask check --locked --offline`
and strict workspace Clippy passed. VB-053's bounded geometry-restore
acceptance is now verified; that does not qualify the broader release surface.

VB-048's bounded opt-in editable/selection capture acceptance is now verified.
The new selection-only production-window journey selects offsets 3–12 in a
synthetic editor, crosses fixed one-use consent, the native collector, Qt
MHTML save and durable object worker, then reopens the exact committed
sidecar. It retains the two offsets and untruncated selection coverage but
no editable value or editor text; the separately verified editable-only
journey retains one bounded draft. `cargo xtask test archive` passes all 21
native journeys. The test harness also follows a visible bounded durable-
flush retry during normal secondary-window close instead of treating that
prompt as an immediate failure. The archive itself is not sanitized, arbitrary
sites are not claimed safe, and VB-050/VB-057 release qualification and the
production gates remain open. Strict workspace Clippy and the complete
`cargo xtask check --locked --offline` passed on the 21-journey source,
including 177 QML passes, seven gated skips and workspace Rust suites. See
`VB-048.md` and `VB-050.md`.

VB-054 now has a continuous committed-draft-to-live-refill native journey.
A synthetic source input receives an unsaved value, one-use consent and the
collector/Qt MHTML/object-worker path publish a durable sidecar and archive,
and a fresh pristine restore window presents a local editable review. The
destination field remains blank before explicit approval; after one confirmed
review it receives the saved value without input/change/focus/blur events or
the fixture's blur-autosave attempt. Contenteditable text remains a
display-only draft, and the original editor fixture is isolated from this
case. All 22 native archive journeys and the complete `cargo xtask check
--locked --offline` pass on the final source (177 QML passes, seven gated
skips and workspace Rust suites). VB-054's bounded acceptance is verified;
arbitrary-site reconstruction, VB-057 release qualification and production
capture/recovery gates remain open. The forced runtime-QML compilation crash
is a separate unresolved startup path. See `VB-054.md`.

VB-055 now has a continuous native committed-media restore journey. The
one-second loopback WAV fixture was made seekable with bounded HTTP byte-range
responses after the first native run showed that Chromium loaded metadata but
refused an in-page seek from a full-only response. Focused fixture tests pass.
The production-window journey captures a paused ~0.4-second position through
fixed consent, the real collector, Qt MHTML and the durable worker; profile
reopen finds the sidecar and a fresh restore window reaches that position
while still paused with zero play/pause events. All 23 `cargo xtask test
archive` journeys pass with metadata-only preload, as do the complete
`cargo xtask check --locked --offline`, strict workspace Clippy and formatting
check on the corrected source. VB-055 remains in progress because the persisted
unkeyed media-source fingerprint is dictionary-testable/linkable and needs
an explicit product/security decision or reviewed key lifecycle; arbitrary
site/media-device behavior and VB-057 also remain open. Production gates
stay closed. See `VB-055.md`.

VB-049/VB-050 now include a negative native frame-interval journey. A live
production-window capture first accepts the same-origin child in its sidecar
and requests Qt MHTML save; a test-only page action removes that child before
completion. The capture retires, and a fresh durable profile reopen finds no
mixed-document commit. The positive same-origin/cross-origin frame journey
still publishes as before. All 24 `cargo xtask test archive` journeys pass,
as do the complete `cargo xtask check --locked --offline`, strict workspace
Clippy and formatting check on this source.
This does not qualify same-URL child replacement, arbitrary frame trees or
VB-057 release, and production capture remains disabled. See `VB-049.md` and
`VB-050.md`.

The VB-049/VB-050 frame interval now also has a same-URL child reload native
journey. After two-frame sidecar acceptance and Qt save request, the iframe
reloads its existing URL; a new child load event is observed, capture retires
and fresh durable profile reopen lists no commit. All 25 archive journeys,
the complete `cargo xtask check --locked --offline`, strict workspace Clippy
and formatting pass. One preceding archive-host startup probe rejected its
own local fixture before any new journey ran, then passed on immediate rerun
and in the full check; do not treat this intermittent result as a release
pass. Arbitrary frame trees, projection-leak checks and VB-057 remain open;
production capture stays disabled. See `VB-049.md` and `VB-050.md`.

VB-049's frame callback now refuses any unreviewed renderer JSON field before
merging its nested frame/candidate/control/editable records into the sidecar.
Synthetic Qt callback injection (including a categorically excluded malicious
child), Rust nested-schema rejection and the application's one-use sidecar
handoff all pass focused tests; a rejected
command/URL-bearing frame leaves no retained sidecar bytes, summary or
interval, and a clean response still admits. The positive live frame tree
passes `cargo xtask test versioning-geometry`. This narrows the frame-result
authority boundary, but arbitrary native frame trees and a complete ordinary
projection audit remain open; VB-049/VB-057 and production capture stay gated.

VB-049's positive production-window frame journey now also inspects ordinary
status, user-facing diagnostics and command history after accepting the
two-frame sidecar. The child path/title/route fingerprint are absent there,
while fresh durable sidecar read confirms the exact child fingerprint was
privately committed. Core summary/Debug tests exclude that fingerprint too.
All 25 `cargo xtask test archive` journeys and the focused core regression
pass. This is one fixed-tree, selected-projection check, not a complete IPC or
arbitrary-tree audit; VB-049 and production release stay gated.

VB-049 is now verified for its scoped bounded-frame implementation. A finite
six-frame native branched tree proves route-based parent relationships across
two same-origin branches and three grandchildren while excluding one
cross-origin child; the depth/count and change-race fixtures remain green.
A new regression initially showed Rust accepting an extra field inside an
editable value; `EditableValue` now denies unknown fields and the regression
passes. The QML callback, Rust parser, one-use application handoff, selected
live public projections and code review of ordinary IPC/query paths together
close the slice's tested metadata/command boundary. The final formatted
`cargo xtask check --locked --offline` passed with 178 QML passes, eight
environment-gated skips, all 25 native archive journeys and workspace Rust
tests; strict workspace Clippy passed. This does **not** enable production
page capture, qualify cookies/WebRTC, or complete VB-050/VB-057 release work.
See `VB-049.md` for bounded evidence and limitations.

VB-050's continuous native capture journeys now verify the persisted
manifest provenance after a fresh durable-profile reopen. The 25-journey
archive campaign passes with assertions for the actual Qt runtime fingerprint,
collector helper version or explicit absence, Qt MHTML backend, paired
wall/monotonic intervals and requested category evidence. The stable page
must retain observed-stable, untruncated complete-for-request coverage;
mutation, navigation, cancelled save and collector/sidecar failures keep
their separate partial, archive-only or no-commit outcomes. This strengthens
VB-050 but does not close VB-045's archive-failure matrix or VB-057 release
qualification, and production capture remains disabled. See `VB-050.md`.

VB-045 now has one continuous production-window archive-copy fault journey.
After the real collector and Qt MHTML save, a test-only one-shot `ENOSPC`
occurs after the first temporary-object chunk. The worker records the fault;
the capture retires and fresh durable-profile reopen finds no commit. The
focused storage regression separately proves temporary-file cleanup and no
installed/orphan object. All 26 native archive journeys pass. This is
deterministic fault injection, not physical disk exhaustion, and does not
address the still-unimplemented multi-tab one-page failure case or VB-057
release qualification. Production capture stays disabled. See `VB-045.md`.

VB-042 is now verified for its scoped consent/policy acceptance. The
application suite covers declined consent, one-use editable/selection flags,
revision-changed scope, native-only command admission and both private and
ephemeral refusal. Offscreen QML tests confirm exact dialog routing and that
the panel warns of secrets, resource URLs and lack of sanitization while the
production action remains disabled. A new real-window journey declines one
operation, refuses its later acceptance, then takes a fresh disclosure to the
only durable capture; all 27 native archive journeys pass. Strict affected-
crate Clippy passes. This does not close VB-045's multi-page failure policy or
VB-057 release qualification. See `VB-042.md`.

VB-043 is now verified for bounded single-page capture staging and asynchronous
blob I/O. The storage suite passes 194/194, including bounded same-kind
admission, exact completion routing, chunked-hash cancellation, quota
reservation and an independent metadata query completing while a 4 MiB
archive copy is paused. A feature-gated, bounded native pause after the first
archive-object chunk leaves the real Qt window responsive to state probes;
resuming the worker publishes the same one-use capture. All 28 native archive
journeys pass. The production archive gate stays disabled; VB-045's
multi-page failure policy and VB-057 release qualification remain open.
See `VB-043.md`.

The 2026-10-04 VB-044 source trace and disposable native check found no safe
public target-redirection workaround for a refused auto-started page save:
Qt retained the original stage path and created it despite the attempted
setter changes. That variant was removed. The reverse-order refusal journey
still fails; keep VB-044/VB-057 open and the production capture gate off. See
`VB-044.md`.

VB-045's object-publication boundary now checks that the single-page
manifest's logical page and window identities equal the one authorized
scope member. Wrong-page and wrong-window manifests are refused before the
operation advances; the valid manifest can still publish afterward. The
focused application capture suite passed 32/32 and strict application
Clippy passed. This does not implement multi-tab partial capture. See
`VB-045.md`.
The full native archive rerun exposed an intermittent Qt save delay after
same-URL child-frame reload: the 45-second test wait expired while the save
remained active. A focused rerun passed quickly once and reproduced the slow
case once. The negative journey now waits through the host's existing
90-second fail-closed deadline; its slow path passed in 91.39 seconds with no
visible commit on durable reopen. A complete `cargo xtask test archive` rerun
then passed all 28 journeys on the fast path. This does not repair Qt's
callback or qualify the full page-continuity release; see `VB-045.md`.

VB-045 now has a schema-3 partial-window result format: the capture request
records whether partial pages were explicitly allowed, and each failed page
has a fixed, payload-free archive reason. Fresh publication rejects missing
or contradictory per-page results, strict-policy failures, and all-failed
commits. Schemas 1 and 2 migrate in memory without accepting claims about
newer policy/failure fields. A real object-backed two-page storage fixture
atomically published one archive and one timeout result, then read the same
commit after profile reopen. Commit detail now exposes the failed page's
opaque logical ID and reason. A distinct two-page one-use consent now binds
the partial policy with fixed disclosure; tab capture remains strict. Native
multi-tab orchestration remains to implement, and production capture remains
disabled. See
`VB-045.md` and `VB-093.md`.

The next VB-045 step added separate fixed-name staging targets for additional
window pages under one operation. The file-only store and worker refuse
duplicate or out-of-range targets and verify each staged page independently
before immutable-object installation. Two-page store and worker regressions
passed, including refusal of a substituted page-target symlink. Application
staging now reserves the whole consented window's per-page worst-case budget
before the first save; a two-tab regression inspected its durable marker.
The application now binds each indexed stage and installed archive receipt
to the matching captured page, records a closed failed-page reason only under
partial consent, and atomically submits an aggregate window manifest only
when every requested page has an explicit outcome. Strict two-page and
partial one-failed/one-saved paths published real objects in application
tests; a changed document was marked failed while another page published.
Reordered/missing page results are refused. The full storage/application
suites passed 199/199 and 166/166, strict storage/application Clippy and the
Qt integration compile passed. Per-page native sidecar work, Qt tab
scheduling/callback correlation, and the multi-tab release matrix remain
open. Keep VB-045 in progress and production capture disabled; see
`VB-045.md`.

The VB-045 native adapter now advances indexed window pages through one
Rust-held manifest and one final publication, with exact QML tab-view lookup
and same-operation callback quarantine. The fixed partial policy can record
an exact native timeout, cancellation, interruption, empty result or missing
view while unaffected pages continue; strict and uncorrelated failures still
abort. Focused Qt Rust tests published two synthetic native archives, then a
timeout plus a synthetic archive, each as one durable window commit. The
ChromeInteractions QML callback suite passed 38/38 and focused application
partial-window tests passed 3/3, including a changed child frame with a live
top-level document. This is not a real two-tab Qt save or full release matrix.
Per-page sidecars are intentionally unavailable until their indexed ownership
is implemented, and the production archive gate remains disabled. See
`VB-045.md`.

VB-045 now has real two-tab Qt evidence. A strict consent published two
different live MHTML archives in one durable window commit. A distinct partial
consent cancelled one exactly correlated live page-save, then published the
other page's real archive with a fixed `cancelled` result for the failed page.
The first partial run exposed a file-only staging bug: Qt removed the
cancelled page-zero leaf, and next-page preparation wrongly required it.
The store now validates the private operation directory while allowing that
first leaf to be absent; a surviving symlink is still rejected. Focused
storage and Rust callback regressions passed. `cargo xtask test archive`
passed all 30 native journeys after the fix, including a 91.41-second
same-URL child-reload deadline path. Window-page sidecars and the remaining
real multi-page failure matrix are still open; production capture remains
disabled. See `VB-045.md`.

VB-045 now binds window-page sidecars to indexed immutable archive receipts.
The application accepts an installed sidecar only for its captured page,
validates its helper version and category summary against final per-page
evidence, and constructs the manifest object reference itself. The native
adapter waits for the exact sidecar install result before advancing. Same-route
window tabs now receive distinct one-use collector challenges; a synthetic
two-page regression rejects reuse of page one's sidecar on page two and
reopens a commit with two distinct sidecar objects. An application regression
rejects a sidecar against another page's archive receipt. The current-source
`cargo xtask test archive` passed all 30 native journeys, including a real
strict two-tab commit with both sidecars. A partial-cancel journey retains
its successful archive with page fidelity unavailable when its sidecar is
absent. Remaining window-specific navigation, timeout, storage-full and
one-page-failure qualification, plus the release matrix, keep VB-045 in
progress and production capture disabled. See `VB-045.md`.

VB-045's real partial-window navigation journey now passes. It navigates the
first captured tab during a live Qt page save and reopens one durable window
commit with a fixed `document-changed` failed first page and the unaffected
second page's verified MHTML object. The first run exposed that Qt may emit
the initial save-request callback only after navigation; the generic stale
request rejection cancelled the entire capture. The exact changed request
now quarantines its staged path and download ID before Qt cancellation,
records the failed page only under the one-use partial consent, then advances.
Completion-time mutation evidence also compares against the URL observed
when the save began. ChromeInteractions QML passed 39/39; the current-source
`cargo xtask test archive` passed all 31 native journeys, including strict,
cancelled-page partial and single-page navigation regressions. Window-specific
timeout, storage-full and further failure cases plus VB-057 release
qualification remain open; production capture stays disabled. See
`VB-045.md`.

VB-045's real two-tab timeout and archive-copy storage-full journeys now pass.
The timeout journey triggers the production 90-second deadline cleanup while
the first save is pending and reopens one durable partial commit with a fixed
`timeout` failed page plus the unaffected real MHTML archive. The disk-full
journey injects one `ENOSPC` in the file-only archive copy; a closed typed
failure reaches the exact install ticket, and the reopened manifest has a
`storage-full` failed first page plus the unaffected second archive. Strict
and single-page failure paths remain all-or-nothing. ChromeInteractions QML
passed 40/40 and `cargo xtask test archive` passed all 33 current-source native
journeys. These deterministic triggers do not prove a 90-second wall-clock
expiry or physically exhausted filesystem. Strict affected-crate Clippy,
formatting, pack validation and checksums passed. Further VB-045 fault qualification
and VB-057 release checks remain; production capture stays disabled. See
`VB-045.md`.

The next partial-window close attempt exposed a real failure: closing the
actively captured first tab removed its live logical-page binding before the
aggregate manifest publication, so the native journey published no commit.
The app now freezes ordered window/page IDs at consent and checks published
IDs against those exact values. Partial publication accepts a now-absent page
only when its consented failure was recorded or its immutable archive was
already installed; strict capture remains all-or-nothing. A focused app run
passed the initial view-loss/frozen-ID regression (3/3), and QML callback tests
passed 41/41 before the latest fixture expansions. The app fixture now also
covers closing an already archived page with another page still live; native
fixtures cover partial and strict view loss plus closing a successful earlier
page during the next save. Closing is now deferred after Qt finishes writing
until exact validation/object installation retires the capture; a feature-
gated native fixture attempts close while the object worker is paused. The
partial-continue and strict-cancel outcomes now report fixed status text with
no page data or private path. Those latest changes remain unverified per the
user's end-of-goal verification instruction. VB-045 remains in progress and
production capture remains disabled.

The next authored VB-045/VB-056 continuation passes the source-of-truth MHTML
byte ceiling to QML, retires the exact stage path and Qt request before
cancelling as soon as reported save bytes cross it, and uses the same cleanup
at completion when Qt progress was unavailable. The Rust end-of-save byte
check remains authoritative, and an application-level callback fixture
rejects a reported size one byte over the maximum before object installation.
QML fixtures cover the inclusive boundary, first-byte overshoot, completion
without progress, partial continuation and strict cancellation; none of these
new tests have been run. This is not a hard physical quota, and archive
capture remains disabled pending the deferred final verification and release
gates.

VB-026 recovery/admission had a source-level gap: its stage scanner rejected
the canonical `capture-page-NNN.bin` leaves created by VB-045 multi-page
capture. Recovery now recognizes only canonical page ordinals in the configured
range and permits the bounded maximum set of page/support leaves, retaining
the existing owner, mode, file-type and hard-link checks. Authored storage
regressions cover unreserved and active-reservation excess accounting, restart
quarantine, the first/last valid ordinals, and malformed/out-of-range names.
These changes and tests remain unverified under the user's end-of-goal
verification deferral; VB-026 stays
in progress. `VB-026.md` records the evidence boundary.

The 2026-10-04 VB-026 continuation adds exact-operation staging retirement
after the object-backed SQLite publication commits. Cleanup accepts only the
bounded module-owned leaves and rechecks their names and Unix identities before each
unlink; it then syncs and removes the operation directory. A cleanup refusal
returns a durable-commit receipt with `staging_cleanup_complete = false`, so
the UI can report that cleanup needs recovery while any files still present
remain included in live budget accounting and restart quarantine. Failed
SQLite publication does not enter cleanup. Authored regressions cover
multi-page source removal with immutable-object preservation, idempotence,
retry after partial leaf retirement, unknown-leaf refusal, committed publication
with refused cleanup, the existing SQLite FULL rollback keeping its source
stage, and killed publication boundaries before/after leaf removal and across
operation-directory sync/removal and staging-parent sync, followed by restart
recovery. Application routing now treats a successful publication receipt as
durable success even when staging cleanup is deferred, releases the in-memory
reservation, and avoids scheduling an abort against the committed operation.
An end-to-end regression injects an unknown staging leaf and checks commit
completion, leaf retention and absence of an abort request; it is authored but
unrun. Do not call these
regressions passed: verification remains
deferred until implementation slices are complete. Path rechecks do not exclude
a concurrent same-UID writer between check and unlink; fsync/power-loss and the
broader VB-091 fault/quota campaign remain open. VB-026 remains in progress,
and production capture stays disabled.

The 2026-10-04 VB-055 feasibility recheck traced the independent capture and
restore request boundaries. Neither has profile secret access, and the
versioning path has no OS credential-store/key provisioning, backup, rotation,
revocation, or recovery contract. A key stored beside the sidecar would not
protect against a profile-directory reader; passing a persistent key through
ApplicationWorld would expand the trusted boundary without resolving custody
or export behavior. No ad hoc key was added. Keep optional media restore
gated until product/security accepts the existing sidecar-only risk or a
separately reviewed OS-protected key lifecycle defines failure and recovery.
See `docs/testing/versioned-browsing/VB-055.md`. VB-024 also now has an
authored-but-unrun regression for a fragment-only same-document change after
restore commit but before the asynchronous sidecar read returns; see
`docs/testing/versioned-browsing/VB-024.md`. VB-051's native archive fixture
now also contains an authored-but-unrun synthetic MHTML case with unresolved
remote CSS, image, iframe, and POST-form references; a native click attempts
the form. Fixed in-memory interceptor counters require CSS, image, and
subframe requests to reach the production blocker, without retaining URLs or
payloads; counts reset for each exact archive authorization, and the fixture
reports whether form handling reached the QML view or the profile blocker.
While that archive is open, a separate disk-backed source-profile view must
also load a loopback control page without replacing the archive; its storage
and cache stay inside the disposable operation directory, and unresolved
archive resources must still produce zero server requests. This is not the
Ferric production profile or a visible multi-window lifecycle qualification.
All paths require zero archive-network traffic and blank acknowledgement. See
`docs/testing/versioned-browsing/VB-051.md`. These fixtures strengthen
acceptance evidence but do not clear the respective
qualification gates. Other independent in-progress slices remain the active
implementation path; final product verification is still deferred by user
request.

The 2026-10-04 VB-026/VB-056 storage continuation adds explicit pre-sync
failure-injection boundaries before operation-directory and staging-parent
sync after exact source-leaf retirement. An authored regression requires the
SQLite commit and immutable object to survive either refusal, then checks
restart recovery against the exact leftover directory state; the process-kill
matrix also includes both boundaries. These are pre-sync gates, not actual
kernel sync errors or power-loss evidence, and have not been run. See
`docs/testing/versioned-browsing/VB-026.md` and `VB-056.md`.

The 2026-10-04 RTC draft's synthetic `screen:-3:0` route has two passing focused
fixtures: one test-only callback invokes Qt's `MediaStreamUI::OnStarted` to
inspect pending-to-active handoff and counter cleanup; the page-side legacy
`getUserMedia` fixture selects only Chromium's fake desktop source and observes
a nonzero track frame, live-capture refusal and stop cleanup without opening an
OS screen. The disposable admission overlay's pending-count saturation,
`INT_MAX`, and unmatched-stop cases also passed in one default and one strict-
site-isolation run. The frame is checked with `MediaStreamTrackProcessor` to
avoid relying on offscreen compositor presentation. This remains a narrow
research route, not a complete media activity observer. VB-033 and production
live-call admission remain blocked. See
`docs/testing/versioned-browsing/VB-033.md` and the spike README.

The 2026-10-04 download-origin source recheck found that the source-locked Qt
6.11.2 `QWebEngineDownloadRequest::page()` returns a page only for WidgetsClient;
the QML adapter path returns null. The QML `view()` getter is not safe for
arbitrary viewless callbacks because its implementation dereferences the
adapter client without a null check. For the authorized archive-save case,
the application now uses the proposal's alternative: a bounded same-profile
owner-tree slot serializes page captures, then correlates the profile-wide
callback by exact private one-use staging path before touching `view`. Rust
records exact permit/path correlation, not a fabricated page-pointer match.
Authored QML and native regressions cover the getter-free path, duplicate/path
ambiguity, profile serialization and an actual MHTML save; they remain unrun
under the final verification deferral. General viewless ordinary downloads
remain unqualified, and production capture stays disabled pending final
verification. See `docs/testing/versioned-browsing/VB-024.md` and `VB-044.md`.

The 2026-10-04 archive callback continuation closes a stale-request gap:
download-ID tombstones are now checked before an old `downloadRequested`
callback can fall through to normal download handling, and the bounded
same-profile owner-tree lookup recognizes retired IDs or private stage paths
without reading Qt's `download.view`. Focused QML regressions were authored
for repeated IDs, retired-path routing, profile scoping, and a throwing view
getter; they are not run under the user's final-verification deferral.
Arbitrary unpermitted viewless Save Page callbacks, native delivery after
owner teardown, and production release qualification remain open. Continue
independent slices; do not enable production archive capture. See
`docs/testing/versioned-browsing/VB-045.md`.

The 2026-10-04 archive interceptor diagnostics now saturate their fixed
per-resource counters at `quint64` maximum rather than wrapping. Compile-time
boundary assertions are authored, but no build or test has run. This keeps
diagnostic counts bounded without changing request authorization or claiming
CSS/image/frame/form isolation; VB-051 remains in progress and production
archive viewing stays disabled. See `docs/testing/versioned-browsing/VB-051.md`.

The 2026-10-04 VB-026 storage pass found that bounded GC refused profiles
above 100,000 commit/object-root entries, while capture publication could
still create more. Capture reservation and commit publication now enforce
that commit ceiling, and publication preflights the projected distinct object
roots so existing shared objects are not double-counted. Authored tests cover
the production row limit, profile scope and incoming-root reuse; they remain
unrun under the final verification deferral. This does not bound SQLite/WAL
physical growth or close same-UID filesystem races. See
`docs/testing/versioned-browsing/VB-026.md`.

VB-052's archive title and shared URL/text sanitizers now cover all Unicode
`Bidi_Control` code points, including U+061C and U+200E–U+200F as well as
overrides and isolates. Rust, source-boundary and native MHTML title
regressions are authored but unbuilt and unrun; exact ranges are cited in
`docs/testing/versioned-browsing/VB-052.md`.

The latest VB-026 storage regression also pins a read snapshot from a second
connection and requires profile flush to fail explicitly while that reader
blocks WAL truncation, then succeed after the reader releases it. Authored but
unbuilt and unrun under the end-of-goal verification deferral; it does not
claim a hard WAL cap. See `docs/testing/versioned-browsing/VB-026.md`.

The 2026-10-04 VB-026 continuation reserves one GC commit slot per active
capture, counting durable commits and profile-local staging operations
together. Publication keeps an independent durable-row backstop. The authored
`ProfileStore` regression requires the final slot to be reserved and published
despite another profile's active capture, a second same-profile capture to be
refused without extra operation/journal rows, and a simulated pre-existing
operation to be refused at publication beyond the cap. It remains unrun under
the user's verification deferral. Physical SQLite/WAL limits and same-UID
filesystem races remain separate. See
`docs/testing/versioned-browsing/VB-026.md`.

The 2026-10-04 VB-056 diagnostic audit found no production logging of capture
payloads; matching print calls are test crash-boundary markers. Commit detail
IPC now uses an isolated allowlist serializer and UI status projections use
fixed diagnostics. Authored storage and serializer regressions require
`CommitInfo` debug and JSON output to exclude source URL, title, capture
message and provenance sentinels even when an archive failure is present.
They remain unrun under verification deferral; native and integrated
failure/privacy qualification stays open. See
`docs/testing/versioned-browsing/VB-056.md`.

## Storage completion debug redaction — authored 2026-10-05

`StorageCompletion` now formats only its closed operation kind rather than
recursively exposing session/library/versioning payloads or error strings;
`StorageOperationError` debug output redacts its dynamic message while keeping
the operation and failure kind. New session-name and private-path sentinel
coverage is authored but not run. This does not change the authorized payload
consumer or the error `Display` contract. See
`docs/testing/versioned-browsing/VB-056.md`.

The 2026-10-04 VB-056 callback audit found that malformed native completion
JSON returned before cancelling its matching capture, leaving the staging
reservation until timeout. The Rust bridge now cancels only that exact
operation; a stale ID cannot cancel the active capture. The focused regression
is authored but unrun under verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

The 2026-10-04 VB-026 journal writer now accepts a closed event enum instead
of free-form kind/summary text, so page-derived data cannot be supplied to
those persisted summaries. Capture publication retains only fixed copy, its
validated operation ID and manifest timestamp. The added assertion is
authored but unrun; dynamic log/IPC and physical SQLite/WAL qualification
remain open. See `docs/testing/versioned-browsing/VB-026.md` and
`VB-056.md`.

The following VB-026 continuation sets an explicit 1,000-frame SQLite WAL
autocheckpoint and 16 MiB retained journal size for each normal profile
connection; automatic checkpoints are PASSIVE and do not truncate the WAL,
while orderly flush uses `wal_checkpoint(TRUNCATE)`. Authored regressions check
those settings, a bounded 20 MiB write across the auto-checkpoint threshold,
explicit-flush truncation and reopen durability. Final verification is pending.
This is not a hard cap before explicit truncation, during a transaction or while
checkpointing is held busy. See
`docs/testing/versioned-browsing/VB-026.md`.

The 2026-10-04 VB-056 privacy continuation also suppresses full Qt QML warning
text in the opt-in stderr hook, reporting only a count. An authored
presentation-boundary source regression protects that behavior. The Qt
warning signal remains connected; this does not cover other Qt/Chromium
diagnostic or crash-reporting channels. The updated source regression and
runtime behavior passed the 2026-10-05 current-tree `cargo xtask check
--locked --offline` and workspace verification. See
`docs/testing/versioned-browsing/VB-056.md`.

The 2026-10-04 VB-052 archive chrome now sanitizes recorded source-URL
metadata before QML display, redacting known credential/token/fragment data
and neutralizing bidi controls while keeping it inert. The adapter regression
is authored but unbuilt and unrun; native isolation and release gates remain
open. See `docs/testing/versioned-browsing/VB-052.md`.

The same VB-052 continuation bounds and sanitizes the archived document title
before it reaches the browser chrome, replacing bidi/control characters while
leaving the saved MHTML untouched. The native fixture distinguishes its raw
WebEngine title from the production component's sanitized display title; the
source and native regressions are authored but unbuilt and unrun. See
`docs/testing/versioned-browsing/VB-052.md`.

**Baseline:** `thomasaiwilcox/ferric-browser`, main `deea458d92bcf6d8fbea7b04af17823b5882cc1b`.

This is an implementation specification, not a patch and not proof that native tests pass. No repository changes have been made by the proposal author.

## Read order

For the first pass, read the repository's applicable agent instructions and `CONTRIBUTING.md`, then proposal Sections 1–9, 23–25 and `BASELINE_AUDIT.json`. Do not treat old chat examples as normative when this proposal specifies a more precise boundary.

For subsequent passes, read the selected task, its referenced proposal sections, its dependency evidence and the current source files. You do not need to reread all task descriptions every time.

The bundle contains:

| File | Purpose |
|---|---|
| `PROPOSAL.md` | Full design, API/source appendix and all implementation slices |
| `IMPLEMENTATION_BACKLOG.json` | Canonical machine-readable task status/dependencies/acceptance criteria |
| `IMPLEMENTATION_SLICES.md` | Standalone human-readable task appendix |
| `CAPABILITY_GATES.json` | Initial primitive/API gate status, separate from production enablement |
| `BASELINE_AUDIT.json` | Pinned repository/source inspection record and known realignments |
| `validate_pack.py` | Standard-library-only ledger validator and next-task/task reader |
| `VALIDATION_REPORT.json` | Author-time pack integrity result; not a Ferric test result |

## Start in goal mode

Suggested goal instruction:

> Implement Ferric Versioned Browsing according to PROPOSAL.md. Advance through dependency-ready slices in IMPLEMENTATION_BACKLOG.json continuously without per-slice checkpoints. Add focused tests and evidence as implementation proceeds, defer executing verification until the final campaign, and update evidence and ledger status without claiming unverified work as verified. Respect capability gates and never weaken architecture/privacy or use private Qt APIs to bypass a blocker. Continue independent work when a conditional capability is unavailable. Do not call blocked cookie snapshot/merge capabilities implemented. At completion, report changed files, actual verification results, and remaining capability blockers.

Optional read-only helper commands, run from this bundle directory:

```sh
python3 validate_pack.py
python3 validate_pack.py --next
python3 validate_pack.py --task VB-000
python3 validate_pack.py --summary
```

The helper does not modify source, update task statuses, install dependencies or run Ferric. It validates the pack and extracts the task to reduce context overhead.

## Per-slice execution loop

1. Confirm repository HEAD, local changes and applicable instructions. Do not overwrite unrelated work. If current main differs from the pinned baseline, inspect the changes affecting this slice and update the baseline evidence. Do not blindly revert to the proposal SHA.
2. Select a task whose dependencies are verified and whose primitive capability requirements are satisfied. If a gate is unsupported, mark that conditional task blocked with the exact evidence and proceed to another eligible task.
3. Read only the necessary proposal sections and source around the named responsibilities. A proposed file path is not a claim that it exists at baseline. Some paths are created by earlier slices.
4. Write a focused test or evidence fixture before the implementation where practical. Demonstrate the relevant failure rather than adding a passing test around a stub.
5. Implement the smallest cohesive change. New domain state goes through core reducer events; orchestration belongs in `BrowserApplication`; storage goes through its coordinator/worker; Qt/QML translate and execute engine operations.
6. Run focused tests and applicable checks unless the current user explicitly defers verification. Under that instruction, author the fixtures now and execute them in the final campaign after implementation slices are addressed. Record unavailable environments distinctly. A native lifecycle, cookie or archive guarantee requires a native test, not `qml.contains(...)`, a mock or successful compilation.
7. Update `docs/testing/versioned-browsing/<ID>.md`, the repository requirements traceability, and the ledger. Review the diff for secrets and unintended persistence. Commit only if the surrounding agent/environment authorises repository commits.
8. Report the next eligible task and continue within the allowed goal-mode execution environment. Do not leave hidden work, indefinite waits or a claim that research will finish later.

A task may be split into smaller suffixed child IDs when necessary, provided the parent is not marked verified until its full acceptance criteria are met and the dependency graph stays explicit. Do not replace one narrow task with a general refactor of `BrowserUi` or the root QML files.

## Status rules

`not-started`: no implementation claim.

`in-progress`: work exists but acceptance criteria/evidence are incomplete.

`verified`: the task's actual acceptance criteria have passed and evidence records the implementation SHA and test commands. For a **qualification** task, a documented negative result may be a valid completion of the investigation; it leaves the capability unsupported and downstream production tasks blocked.

`blocked`: a concrete missing API guarantee, environment, dependency approval or architectural decision prevents completion. Store a reason, evidence path and exact condition for unblocking. Do not mark it verified merely because an Unsupported return was added, unless the task specifically implements the unavailable path.

Primitive gates and production feature enablement are separate. A small positive API probe permits the corresponding integration work; it does not turn on the finished feature. Production commands require the release-milestone qualification as well. A known negative general-cookie result cannot be overridden by an ordinary-cookie unit fixture.

If no eligible work remains, report the supported implementation achieved and the outstanding blocked extensions. “Execution complete with blockers” is not “the entire product is implemented.”

## Non-negotiable constraints

- No private Qt/Chromium APIs in the production adapter; only the explicitly authorized isolated native capability spike may patch engine internals. No V8 snapshots, process checkpointing, Chromium SQLite reads or copied live profile directories, including in the spike.
- No remote-debugging endpoint as an unreviewed cookie workaround. CDP requires a separate approved backend/security decision.
- No implicit private/ephemeral durability; branch export is a specific confirmed exception, not normal-profile fallback.
- No raw cookie data or editable contents in logs, JSON IPC, command history, normal journals or test snapshots.
- No mutation outside the reducer/application ownership boundary and no native callback retargeting based on current focus.
- No mixed-profile live tab reparenting. Initial branch hosts are separate native windows aligned with current window/profile invariants.
- No assertion that Frozen is immutable, no silent reload masquerading as LIVE, and no automatic eviction of the only live copy.
- No assertion that a timer/sentinel/mirror revision provides an atomic complete cookie snapshot.
- No blind whole-jar merge/rollback. Unsupported identity or concurrent-write guarantees block production application.
- No actual external account tests without explicit authorisation; CI uses synthetic local fixtures.

## Existing repository verification commands

The inspected `CONTRIBUTING.md` documents:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo xtask check --locked --offline
cargo xtask test adapter
cargo xtask test wayland
```

Use focused `cargo test -p <affected-crate> --locked --offline` runs while iterating, then the appropriate complete gates. Discover the real xtask fixture layout in VB-000. Do not install Weston/Qt or use `--no-sandbox` to turn an unavailable environment into an apparent pass.

The 2026-10-04 ordinary-transfer route now avoids Qt Quick's `download.view`
getter for every non-Save-Page request. A new request falls back to the exact
profile-signal owner after exact retained owners are considered; ambiguous
claims still refuse. This is intentionally profile-level download management,
not origin-tab identification, and completion still requires exact retained
request identity. The same check covers the private secondary profile. A
throwing-getter QML case and static source regression were authored, not run.
Unpermitted Save Page can use the profile manager only after a bounded scan
proves every same-profile capture-capable owner is present and no tab is
poisoned; malformed or missing state refuses saves profile-wide. The
secondary private-profile handler has no capture path and is still profile-
bound. Native proof of a truly viewless transfer through durable ledger
completion is still needed. Unowned Save Page requests are tombstoned before
cancellation so late finish callbacks cannot flow into ordinary finalization;
quarantine exhaustion suppresses page saves for that window. Keep
VB-024/VB-044/VB-057 in progress and archive capture disabled; see
`docs/testing/versioned-browsing/VB-024.md` and `VB-044.md`.

VB-029's structural milestone record and proposed release-boundary draft are
now at `docs/testing/versioned-browsing/VB-029.md` and
`docs/testing/versioned-browsing/structural-release.md`: they narrow the
supported scope, map the existing dependency evidence, and list the final
campaign needed for current-tree acceptance. These are not qualification
results; no VB-029 tests or native journeys have been run, and its ledger
status remains in-progress.

The disposable exact-Qt-Quick-view RTC fixture now also authors a synthetic
audio-only microphone route. It holds the QML permission object pending while
the same hidden `WebEngineView` requests an empty-registry lease, requires the
pending request to refuse that lease, then grants only the synthetic source,
checks active-track refusal and waits for empty admission after stopping it
before proceeding to the data-only peer case. It also shows the hidden view,
requires lifecycle-exit lease revocation, hides it again and requires fresh
empty admission before media work. No OS microphone is accessed. This
fixture/source-lock change is unbuilt and unrun. Source review also found the
prototype's lifecycle guard omitted a still-pending acquisition; the isolated
patch now invalidates that state too, and the fixture exercises immediate
visible/Active transition, exact late-result refusal, and fresh reacquisition.
Neither the latest patch nor the expanded fixture has been patch-checked,
built or run. It adds no production capability. Keep VB-033 blocked until the
final native campaign, broader activity coverage and production
ownership/release gates pass.

The exact-view fixture is also authored to swap its leased adapter to a
second off-the-record profile, observe old-lease invalidation, wait for the
replacement page reload, and obtain a fresh empty-registry observation on the
new adapter. This is unrun and remains research-only; prior evidence binds to
the older source/fixture tuple.

The exact-view fixture now also covers a combined synthetic audio/video
`getUserMedia` request. It requires pending refusal while the combined Qt
permission is held, active refusal with both live tracks and a delivered
synthetic frame, and a fresh empty lease after both tracks stop. The combined
leg uses fake devices only. The current fixture/patch tuple remains
unbuilt/unrun, and VB-033 stays blocked with production capability disabled.

The native RTC fixture now also authors Linux-only renderer-loss coverage: it
verifies the exact probe-owned renderer child, opens a pidfd and rechecks
process identity before signalling, then expects exactly one invalidation and
an `Unknown` follow-up observation. This current fixture hash is unbuilt and
unrun; it adds no qualification or production capability claim.

The native RTC fixture now also covers a combined synthetic camera-and-
microphone request. It holds the combined Qt permission pending, verifies
`Unknown` admission, grants fake devices only, checks both live tracks and a
synthetic video frame, refuses while capture is active, and waits for fresh
empty admission after stopping both tracks. A separate fresh off-the-record
page denies the combined request and checks that the pending token clears
before empty admission. This update is authored and unbuilt/unrun; older native
RTC evidence remains bound to its recorded fixture hash and production
capability remains disabled.

The exact Qt Quick owner fixture now also closes its data-only peer, obtains a
fresh empty-registry lease on the same hidden view, then triggers loss of that
view's reported renderer using Linux pidfd signalling only after checking
child/executable identity and rechecking process start time. It requires Qt's
renderer-terminated signal and exactly one lease invalidation from the bridge.
The updated C++/QML fixture pair is authored but unbuilt/unrun; previous
results remain bound to the older hashes and production stays disabled.

Static review of that exact-owner continuation found its repeat-acquisition
state mismatch: native admission begins from hidden `Active` and performs its
own freeze, but later QML legs requested another lease while already `Frozen`.
The disposable Qt Quick probe bridge now transitions only a hidden `Frozen`
owner to `Active` before admission and rejects visible/non-active owners; the
native operation must still return the same hidden owner to `Frozen` before a
lease is retained. This correction is authored, unpatch-checked and unrun. It
does not qualify the native fixture or production activity absence. The QML
fixture now also directly requires synchronous refusal for an acquire called
while that exact owner is visible; this assertion is authored and unrun. The
three acquire/check/release invokables also perform their owner-thread guard
before reading private Quick-view state. The C++/QML runner directly invokes
all three from a foreign thread while the exact lease is held, then requires
the lease to remain valid. This is authored and unrun. Keep VB-033 blocked and
the production capability disabled.

## 2026-10-05 Qt Quick exact-owner capability follow-up

The current zero-fuzz Qt Quick bridge patch was rebuilt into the isolated Qt
engine and passed one fresh `probe-rtc-quick` run. That run exercised exact-view
empty-lease ownership, overlapping and foreign-thread refusal, lifecycle exit
and pending-acquisition cancellation, profile-adapter replacement, data-only
peer registration/close semantics, same-view document destruction, and
renderer-loss invalidation. The closed peer correctly remained registered
while its object remained in the live document; reload/context destruction then
allowed an empty lease. The exact-view `renderProcessTerminated` route released
the lease once despite the racing native observer callback. The Linux fixture
accepted only the reported renderer reached through at most two verified
QtWebEngine zygotes rooted at the probe, then used pidfd and a process-start
identity recheck before signaling.

This is one scoped research run, not qualification. Qt Quick fake camera,
microphone, and combined-permission routes remain disabled because offscreen
hidden-view requests failed before Qt's permission callback. Other renderer,
media/capture, missing-acknowledgement, production Rust ownership, repeated
campaign, and release qualification remain open. VB-033 remains blocked and
all production capability flags remain disabled. Exact patch, fixture, library,
and executable digests are in `tools/versioning-engine-spike/source-lock.json`;
the detailed result is in `docs/testing/versioned-browsing/engine-capability-spike.md`.

## Suggested evidence record

```markdown
# VB-000 — <slice title>
Status: not-started | in-progress | verified | blocked
Baseline SHA:
Implementation SHA:
Affected requirement/invariant IDs:

## Behaviour implemented or investigated

## Tests actually run
Command:
Environment:
Result:

## Native evidence (when required)
Qt / Chromium / security patch:
Compositor / platform:
Fixture version:
Observed behaviour:

## Privacy and ownership review

## Remaining blockers
Reason:
Evidence:
Unblock condition:

## Next eligible task
```

Keep the record bounded and factual. Save synthetic fixtures, not captured production profiles or credentials.

## Ordinary download completion owner drain — 2026-10-04; authored, not verified

Root, restored/secondary, and popup owners now keep a dedicated completion map
from accepted routing through the exact profile-level terminal callback. The
map is intentionally separate from active/chooser UI maps: rejected and
canceled requests remain owner-routable without being presented as active.
Owner resolution checks the exact request object; normal completion removes
the entry only after finalization or staging discard. Shutdown checks this map
without relying on `isFinished`, so a finish flag racing ahead of the profile
signal cannot release the host. Cancel-and-close waits in a visible state and
continues the normal close path after the final callback; Keep cancels that
intent. This handles orderly teardown only and does not certify callback
delivery after forced destruction.

Authored regressions cover the completed-before-signal owner-map case and
source contracts for retaining ownership during shutdown. No tests, build,
lint, formatter, or pack validation were run under the user's verification
deferral. The new route is not verified until the final test/native campaign.
VB-024 and VB-044 remain in progress; archive release is still disabled and
VB-033 remains blocked on production WebRTC-absence proof.

The Qt 6.11.2 public QML contract says `downloadFinished` covers successful,
cancelled, and interrupted ordinary transfers; the existing native two-view
fixture also covers cancellation of a streamed ordinary transfer. Keep this
separate from the native page-save cancellation observation, where a terminal
state arrived without a finish callback inside the bounded wait. The new
cross-check is recorded in `docs/testing/versioned-browsing/VB-024.md`; it was
read-only research and has not been rebuilt or rerun on the current tree.

## Current-tree final verification — 2026-10-05

The final `cargo xtask check --locked --offline` passed on the current tree:
196 QML checks passed, none failed, and eight environment-gated tests skipped;
bounded native archive/capture, cookie, profile-download, HTTPS, release
manifest and license probes passed. `cargo xtask test archive` passed the full
native matrix and durable-adapter follow-on. The independent workspace Rust
run passed 1,228 tests with 30 ignored; strict workspace Clippy, formatting and
final storage-crate Clippy passed. The current aggregate-GC guard and WAL
flush/busy-reader regressions passed.

The old reverse-order diagnostic directly retried the same Qt WebEngine view
after refusal, skipping Ferric's production per-tab quarantine. The controlled
probe now models the exact tab, requires quarantine and blocks same-tab retry
before Qt is invoked. Qt's delayed finish and unsafe view reuse remain negative
evidence; this recheck verifies the fail-closed production policy, not native
view reuse.

Pack status after reconciliation: 44 verified, 11 in progress, 36 blocked,
five not started; `validate_pack.py --next` returns no dependency/capability-
ready not-started slice. All production feature flags remain false. Continue
independent in-progress work, but leave cookie identity, complete live/WebRTC
absence, archive release, exact physical quota, and second-runtime qualification
gates open until their evidence or required backend becomes available.

## Native captured-tab close ordering — 2026-10-04; archive matrix verified

One real SIGSEGV occurred in Qt 6.11.2's profile `downloadRequested` dispatch
while a queued SavePage/MHTML event was being delivered. GDB located the
faulting call in `QQuickWebEngineProfilePrivate::downloadRequested`; the
event's `DownloadItemInfo` carried ID 1 and a private capture staging path.
The evidence is consistent with destroying the source view before the queued
profile event consumed its page pointer, but does not prove that root cause.

The production close path now leaves that captured `WebEngineView` attached
until the matching private path is observed by the profile route. While that
request is pending, QML does not retire the old capture, time it out, or start
a replacement capture. After registration, bounded path/ID quarantine still
precedes view destruction; a completed request continues to block close until
archive installation finishes. The native strict-window and partial-window
close-before-registration journeys passed, as did deferred close during
installation, the paused-worker responsiveness case, and the remaining
`cargo xtask test archive` matrix. Formatting passed. A separate test-fixture
assertion was corrected to check the active second tab while closing the first.

The archive isolation fixture observed zero stylesheet/image/subframe/main-
frame interceptor requests, one source-profile control request, zero
unresolved archive requests and zero network hits. Qt's archive renderer
suppressed those resource loads before they reached the interceptor; this is
not per-category interceptor-block evidence. The native MHTML capture/reopen
journey and the `cargo xtask test archive` matrix passed, but production
archive capture remains disabled: ordinary viewless-download delivery,
reverse-order SavePage cancellation and the broader VB-057 release campaign
remain open.

## 2026-10-05 bounded aggregate GC inspection — authored, not run

VB-026's collector limited object rows and each archive size, but a damaged
index could still make it hash up to 100,000 maximum-sized objects in one GC
pass before discovering the aggregate was too large. The collector now
validates every referenced-ready row's stored size and the aggregate before
opening any object file, refusing if one exceeds the archive limit or the
aggregate exceeds the profile's 2 GiB versioning budget. A focused
exact-boundary/over-boundary regression is authored, not
run under the user's final-verification deferral. A storage-worker regression
also seeds 33 maximum-sized referenced index rows without backing files and
requires refusal with every row still in its original ready state. This
narrows work on a corrupt index; it does not bound physical allocation or close VB-091's
same-UID and filesystem fault campaign. See `docs/testing/versioned-browsing/VB-026.md`.

## Public Qt viewless-download feasibility recheck — 2026-10-05

Qt 6.11.2 documents that `WebEngineDownloadRequest.view` is null when the
request did not originate in view content, and `QWebEnginePage::download()`
emits the profile's public `downloadRequested` signal. Therefore viewless
profile requests are a supported Qt capability, not an unsupported API gap.
Ferric's production `QQuickWebEngineProfile` public API does not expose the
wrapped core `QWebEngineProfile`, however, so a detached `QWebEnginePage`
cannot be attached to the exact Ferric profile without Qt private API. Do not
use that private adapter. Current profile-owner routing and QML
throwing-getter regressions are still not a native viewless-to-durable-ledger
journey; see VB-024.md and VB-044.md.

References: [Qt 6.11.2 QML WebEngineDownloadRequest](https://doc.qt.io/qt-6.11/qml-qtwebengine-webenginedownloadrequest.html), [QWebEnginePage::download](https://doc.qt.io/qt-6.11/qwebenginepage.html#download), [QQuickWebEngineProfile public API](https://doc.qt.io/qt-6.11/qquickwebengineprofile.html).

## VB-056 explicit QML diagnostic redaction — authored 2026-10-05

The QML warning hook was already count-only, but the separate opt-in startup
failure dump still printed every `QQmlError::toString()` to stderr. It now
emits a fixed details-suppressed message and the bounded error count, and the
presentation-boundary regression checks both the normal warning hook and the
explicit component-dump path. A further source audit found that the QML
object-creation failure callback also printed the failed component URL; it now
emits fixed details-suppressed text at most once per launch and ignores the
URL, covered by the same presentation-boundary regression. These changes were
made after the 2026-10-05 full campaign; the source and regression still need
to be run in the end-of-goal verification. External Qt/Chromium logging and crash-reporter
channels remain unqualified. See `docs/testing/versioned-browsing/VB-056.md`.

Qt 6.11's public docs specify that QML warnings are sent to stderr by default.
Ferric now suppresses that raw output on every engine, while keeping its
payload-free warning count opt-in. A source regression protects the
unconditional suppression call and its ordering before the diagnostics flag.
This addition also postdates the full campaign and is pending the final
verification run. Reference: [Qt `QQmlEngine`](https://doc.qt.io/qt-6.11/qqmlengine.html#setOutputWarningsToStandardError).

## 2026-10-05 current RTC research rerun

The latest native RTC fixture passed three fresh default and three strict
`--site-per-process` off-the-record runs. The latest Qt Quick exact-view fixture
passed three fresh runs, including the synthetic fake camera, microphone, and
combined-capture permission/frame/refusal/cleanup legs, lifecycle races,
profile replacement, data-only peer close semantics, and renderer-loss
invalidation. Fixed Chromium fake devices were used; no OS capture devices were
opened. Current fixture/executable/log digests are in
`tools/versioning-engine-spike/source-lock.json` and the test narrative is in
`docs/testing/versioned-browsing/VB-033.md`.

During the native rerun, the renderer-loss guard initially refused Qt's
reported renderer because Chromium inserted a helper process and sanitized
argv. The research fixture now validates only the public page-reported target
PID, the exact isolated QtWebEngineProcess executable, a bounded ancestry of
at most two intermediate helpers rooted at the fixture, and stable process
start identities before and after `pidfd_open`; only then does it signal that
same pidfd. The corrected fixture passed all six native configurations and
observed exactly-once invalidation. This is diagnostic engine-fork evidence,
not production RTC absence proof. VB-033 stays blocked, all production
capability flags remain false, and no not-started slice currently has both
verified dependencies and satisfied capabilities (`validate_pack.py --next`
returns null).

## Automatic session checkpoint isolation — authored 2026-10-05

Normal-profile session snapshot creation and recovery now require a normal
core profile bound at profile setup, a durable profile owner, a destination
inside that profile's session directory, and no child-branch record for that
profile that remains
not explicitly closed (including lost branches). Since exact
branch-to-window/tab ownership is not available, this is a conservative
profile-wide refusal while a child-branch record remains unclosed, including
lost branches; it also temporarily disables snapshots for unrelated windows
in that profile. Recovery applies the same policy before requesting storage
reads. One shared session-access gate also protects named-session save, load,
preview and delete, plus catalog listing. Denied catalog access clears cached
names. Session catalog, restore, save, delete and checkpoint-cleanup requests
retain their storage ticket, and a completion that does not match the active
ticket is discarded; this closes the same-profile-is-eligible-after-switch
case that a fresh access-gate check alone could not detect, including an old
checkpoint-save result incorrectly cleaning the new profile's dirty state.
Cached names are excluded from switcher
results unless the gate still allows access, and restore/preview buffers are
rechecked before QML consumes them. Revoked values are cleared and replaced
with a fixed error for a pending preview instead of being applied. The builder
then projects only tabs the core marks visible for that window, excluding
retained/transferring reducer-live views from recovery. Only one session
restore/preview/recovery operation may be pending per UI owner. Policy/projection helper
tests passed (2/2) before the
later profile-binding, branch-gate, path and diagnostic changes. The authored
profile-binding, branch-lifecycle, one-pending-restore, policy/path-isolation,
stale-result, switcher/consumption and diagnostic sentinel tests remain unrun;
the request-ticket mismatch regression is also authored but unrun. Branch
creation remains capability-gated. See
`docs/testing/versioned-browsing/VB-056.md`.

Session DTO and error `Debug` output is also redacted now, preventing accidental
diagnostic formatting of page URLs, titles, workspace labels, session names or
filesystem paths. Session I/O and decode `Display` messages are fixed and
path-free rather than exposing filesystem paths or raw OS/parser details;
storage JSON behavior is unchanged. `StorageCommand` debug output now contains
only the operation kind, not payloads or paths. New session/storage command
sentinel tests are authored but unrun and should be included in final
verification.

## Disposable cookie destination-owner loss — 2026-10-05; three fresh campaigns passed

The source-pinned native cookie fixture now includes an authored owner-loss
stage: it starts a copy from each safe disk-loaded control source to a fresh
disposable destination, destroys the exact destination profile before its
async native reply is routed, requires one `OwnerLost` result, then proves the
source remains usable by copying it exactly to an off-the-record control.
Runner paths are constrained under the unique disposable root. All three
fresh campaigns passed 32 checks each with zero failures on the pinned
isolated engine. This checks QObject/profile-owner callback retirement only;
it does not simulate a bound-but-disconnected network service, crash
durability, source-generation binding, or a child writer lease. It adds no
production capability evidence. Fixture, runner, library, process, and probe
hashes are in `tools/versioning-engine-spike/source-lock.json`; production
cookie/WebRTC flags remain disabled.

## Disconnected cookie-manager Mojo pipe — 2026-10-06; six fresh cases passed

The source-pinned native probe now drops an in-flight GetAllCookies reply
by disconnecting the bound CookieManager receiver while its backing-store
operation is pending. Three source-loss and three destination-loss cases
passed nine checks each. All six completed exactly once as TimedOut; on
destination loss the source retained two records, destination contents were
unknown, and the possibly mutated child was discarded. Build/install
completed after reducing the isolated Qt build to two jobs. Cookie baseline,
legacy/older-expiry, HTTPS, and current RTC regression controls also passed.

This answers the bounded disconnected-pipe callback question only; it does
not model network-service process crash/restart, exact destination read-back,
source generation, competing writers, crash-durable publication, or complete
WebRTC absence. Production cookie and WebRTC flags stay disabled, and VB-075's
production seed gate remains closed. Full digests, per-case counts, disposable
roots, and the patch-reproducibility caveat are in
`tools/versioning-engine-spike/source-lock.json` and
`docs/testing/versioned-browsing/engine-capability-spike.md`.

## Native page capture in shutdown accounting — authored 2026-10-05

The shutdown path previously counted only ordinary pending/active downloads and
a pending view-close capture, while the admitted native MHTML save is tracked
separately. Active native capture now keeps shutdown's download gate open, and
cancel-and-quit retires exact callback identities, clears the capture owner,
discards its mutation probe, cancels an unfinished Qt request and cancels the
whole Rust capture operation. A completed Qt write is not cancelled again;
its exact callbacks are still quarantined. Two focused QML regressions assert
that ordering. These source changes and regressions are authored but unrun per
the end-of-goal verification deferral. Object-worker shutdown and final release
qualification remain open; VB-045 stays in progress and production capture
remains disabled.

The native archive probe now also explicitly checks that its immediate Qt
cancel case reports `isFinished == true` without `DownloadCompleted` and leaves
no archive output file. This assertion is authored but not yet run; the older
passing native campaign predates it.

Shutdown accounting also now includes a Rust-owned page-capture operation
before QML receives its exact Qt save route. A no-argument Rust adapter method
cancels only the current disclosure/authorization operation, while the QML
helper keeps the existing exact path/request tombstone sequence once the
native save owner exists. Shutdown gating is restricted to the exact root UI,
so an unrelated popup/secondary-window close does not inherit this prompt.
The ownerless route also clears the stale QML consent projection after Rust
confirms cancellation.
Added an authored Rust regression for cancellation
after stage preparation but before QML claims the save, plus a QML helper test
for the ownerless route. These changes and regressions remain unbuilt/unrun
under verification deferral. Update VB-045 after the final verification
campaign; keep production archive capture disabled.

## Object-path diagnostic redaction — authored 2026-10-05

The storage privacy follow-up also removed a contradiction in `ObjectPathError`:
its comment promised no raw filesystem diagnostics while derived `Debug` and
`Display` forwarded `io::Error` text. Both now redact dynamic I/O details but
retain the closed failure kind. A path-sentinel regression is authored and
unrun. See `docs/testing/versioned-browsing/VB-056.md`; include it in the
deferred final verification campaign.

Direct storage-snapshot diagnostics are now count-only too:
`ProfileLibrarySnapshot` and `JourneyQuerySnapshot` no longer recursively
format page URLs/titles, command strings, or journey IDs. Their direct sentinel
regression is authored but unrun; see the same VB-056 record.

## Deletion-preview row-scan bounds — authored 2026-10-05

VB-026 deletion-preview child/reference counts and total object-link summaries
now refuse beyond the fixed 100,000-row ceiling. Active-operation and capture-
slot admission counts use the same bounded-row pattern. This closes the
unrestricted aggregate cardinality found during source review; authored
boundary regressions are unbuilt/unrun under verification deferral. This caps
matching rows fed to the summaries, not SQLite page visits for a sparse or
corrupted index, and it is not a storage quota or defense against same-UID
database mutation. Details are recorded in `docs/testing/versioned-browsing/VB-026.md`.

## Manifest object digest debug redaction — authored 2026-10-05

VB-056 now redacts the stable SHA-256 when a `ManifestObjectRef` is formatted
directly, preventing an accidental debug log from becoming a cross-capture
content correlator. Serialization and digest-based storage are unchanged. A
sentinel regression is authored and remains unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

## Restore-plan mutation seal — authored 2026-10-05; not run

`NewWindowRestorePlan` now contains a private SHA-256 integrity seal over all
publicly mutable execution fields, including safe URLs, placeholder reasons,
page metadata, selection, navigation order and admitted revision. Execution
checks it before revalidating the exact private native-owner binding,
revision and URL policy; changing a plan after admission, even to a different
valid safe GET URL, is rejected before state mutation. `NativeProfileOwner`
also has a redacted `Debug` projection so its application/profile-instance
tokens are not emitted; restore integrity never hashes a formatted debug
string. Safe-API mutation detection is not an authorization boundary against
unsafe code or memory corruption. The restore URL-substitution regression
and the native-owner debug sentinel regression are authored but unbuilt/unrun
under verification deferral. See `docs/testing/versioned-browsing/VB-024.md`
and `docs/testing/versioned-browsing/VB-056.md`.

## Publication object-root count bound — authored 2026-10-05; not run

The publication preflight's distinct object-root count was also an
unrestricted aggregate. It now uses a `DISTINCT ... LIMIT maximum + 1` query
and refuses at the first excess root. The deletion preview's correlated
distinct-sharer aggregate now has the same per-object cap and refuses when an
object is shared by too many commits. Exact-boundary/over-boundary coverage
for both paths is authored but unbuilt/unrun. Commit info/list projections
now likewise cap missing/quarantined-state and object-link rows, including a
single shared page-wide listing-row ceiling, and refuse over-limit results.
Budget admission also replaces `SUM` across object rows with a row-capped size
walk that validates per-entry sizes and the aggregate profile budget. Focused
boundary coverage for these additions is authored but unbuilt/unrun. These
bound result rows, not internal SQLite page visits for malformed/sparse
indexes or physical volume; see `docs/testing/versioned-browsing/VB-026.md`.

Journal pruning now preflights bounded journal and terminal-operation row
counts, refusing oversized indexes before mutation; append also refuses before
inserting if the subsequent pruning work would exceed the bound. Its
read-only over-limit regression is authored but unbuilt/unrun.

## Storage error Display redaction — authored 2026-10-05; not run

`StorageOperationError::Display` previously forwarded the same dynamic worker
message that `Debug` already redacted. It now emits only the closed operation
and failure kind, preventing default error formatting from exposing paths or
SQLite details. The explicit message remains available to the authorized
application consumer. The existing name/path sentinel regression also checks
the `Display` projection and is authored but unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

The read-only `StoreInspection` projection also replaces corrupt/unavailable
SQLite details with fixed summaries, and an authored corrupt-file path
sentinel checks both the structured reason and `Debug` projection. It remains
unbuilt/unrun under verification deferral.

## Manifest profile-identity debug redaction — authored 2026-10-05; not run

Direct `ManifestHeader` and `VersionedCaptureManifest` debug projections now
replace the stable profile UUID with a fixed redacted marker. Focused sentinel
coverage checks the manifest and header projections. Serialization and exact
profile identity matching are unchanged; coverage is authored but unbuilt and
unrun under the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

The Qt policy-configuration bridge now converts JSON serialization/encoding
failures to fixed messages rather than returning raw serde diagnostics across
its UI boundary. This path has not been executed under the verification
deferral; see `docs/testing/versioned-browsing/VB-056.md`.

## GC row-cursor bounds — authored 2026-10-05; not run

VB-026's profile GC reference-root and ready-object queries now carry a
`LIMIT maximum + 1` parameter in addition to the existing over-limit guards.
This bounds rows emitted into Rust collections and allows early refusal once
the profile index exceeds the configured ceiling. It does not bound SQLite's
internal page visits or distinct/order work for sparse or corrupted indexes.
An exact-boundary/first-excess SQLite fixture now covers both cursor paths,
including repeated links to shared roots; it has not been built or tested
under the verification deferral. See
`docs/testing/versioned-browsing/VB-026.md`.

The restart-time referenced-object scan, ready-object size projection and
active-capture reservation query now also use `LIMIT maximum + 1`; refusal
occurs before quarantine or budget admission can proceed. An in-memory SQLite
regression covers exact/over boundaries, repeated links to shared roots and
profile-scoped reservations. It is authored and unrun under verification
deferral. This limits emitted rows, not SQLite internal page visits or
physical profile volume; details are in `docs/testing/versioned-browsing/VB-026.md`.

A separate recovery-entry regression seeds an over-limit damaged reference
index and a real private staging orphan, then requires restart recovery to
refuse before creating a quarantine batch or moving either staging path. It
is authored but unbuilt/unrun under the verification deferral. This covers
preflight ordering only, not arbitrary filesystem faults or same-UID races.

## Opaque URI IPC redaction — authored 2026-10-05

The error-response sanitizer previously passed opaque URI payloads such as
`mailto:address?subject=...` through unchanged and retained hierarchical URL
paths, including possible file/archive paths. It now preserves only a
validated scheme and non-file origin; `file:` URLs use a fixed redacted
authority and no path. It peels/restores surrounding quote/bracket punctuation
during detection. A response-level regression checks recipient, subject, body,
HTTPS-path and archive-path sentinels across bare/wrapped error messages and
nested diagnostic context. Authored, not built or run under verification
deferral; broad dynamic IPC/log and lifecycle coverage remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

VB-026 commit deletion and quiescent GC now preflight the total `vb_objects`
index through a `MAX_RETENTION_ROWS + 1` bounded row count. An oversized
damaged index is refused before deletion metadata changes or GC file/state/
quarantine work begins. An exact/first-over SQLite regression confirms the
preflight is read-only. Authored but unbuilt/unrun under the user's final
verification deferral; this bounds returned rows, not internal SQLite page
visits, physical storage, or same-UID/filesystem fault behavior. See
`docs/testing/versioned-browsing/VB-026.md`.

Deletion preview now separately bounds raw `vb_commit_objects` rows for its
target before summarizing them. This catches dangling damaged links omitted by
the summary's join but still included by the commit-keyed delete. An authored
production-preview regression accepts the exact raw-link limit, then refuses
the first dangling excess without mutating those links; it is unbuilt/unrun.
No foreign-key repair or hard SQLite page-visit bound is claimed. See
`docs/testing/versioned-browsing/VB-026.md`.

## Shutdown waits for versioning object-worker retirement — authored 2026-10-05

VB-045's primary and secondary quit paths now check the application-owned
storage coordinator for admitted or queued file-only versioning work after
cancellation. They keep the shutdown prompt/window alive and recheck on the
normal runtime/storage pump until worker completion has been routed; durable
metadata work remains covered by the existing flush. An authored
storage-coordinator regression checks active and queued object jobs, including
file-only capture staging, while excluding metadata publication,
and a Qt/QML source-contract regression checks the wait/re-entry path. These
changes are formatted but deliberately unbuilt/unrun under the user's
end-of-goal verification deferral. The native close-while-worker-paused
journey, OS-level blocking I/O behavior and VB-057 release campaign remain
open; production archive capture stays disabled. See
`docs/testing/versioned-browsing/VB-045.md`.

## Storage/profile-open error diagnostic redaction — authored 2026-10-05

VB-056's direct storage-open errors had a separate leak: `StoreError::Corrupt`
and `StoreError::Sql` derived/formatted their dynamic diagnostic payloads, and
`StorageWorkerError::Spawn` interpolated its stored reason. `ProfileOpenError`
also forwarded filesystem-derived bootstrap strings to ordinary formatting,
and profile-open DTO debug projections included config paths and values.
`ProfileSnapshot` now redacts its profile name, identity and roots. The error
`Debug`/`Display` projections are closed and redacted while
preserving typed error variants and returned data for in-process use.
Private-path/SQL sentinel regressions are authored but unbuilt/unrun under the
verification deferral. Broader dynamic IPC/log coverage and the filesystem
fault campaign remain open; see `docs/testing/versioned-browsing/VB-056.md`.

## IPC and command diagnostic redaction — authored 2026-10-05

VB-056 now replaces raw derived Debug output for IPC requests/responses/events,
protocol/public errors, hello identity, command invocation/envelopes and
routing context. Framing-error Debug/Display no longer emits dynamic I/O or
serde diagnostics; request-cache and instance path/error Debug/Display
projections omit request identities, local paths, owner strings and payloads.
Serialization and authorized in-process values are unchanged. Sentinel tests
are authored but unbuilt/unrun under the user's end-of-goal verification
deferral. Dynamic logs/Qt diagnostics and lifecycle/fault coverage remain open. See
`docs/testing/versioned-browsing/VB-056.md`.

The response-text scrubber now also removes local POSIX/relative/drive-letter
paths and values keyed as paths, files, directories or sockets. The existing
response sentinel test covers private profile and socket paths; this extension
is likewise unbuilt/unrun.

The same pass found `NewWindowRestorePlan`'s custom Debug output exposing the
source UUID and runtime profile identity; those fields now use fixed redaction
markers, with assertions added to the owner-rebind regression. It remains
unbuilt/unrun.

The IPC response scrubber also now recognizes Windows UNC paths, including
extended UNC prefixes, as local paths. An authored sentinel regression covers
UNC and drive-letter forms; it remains unbuilt/unrun under the user's final
verification deferral. See `docs/testing/versioned-browsing/VB-056.md`.

The runtime exception audit found that derived `RuntimeError` Debug output
included navigation URLs through both diagnostic text and the source chain.
Its custom Debug projection now retains the stable code and source-presence
bit while redacting user text and diagnostics. An authored mailto recipient/
query sentinel regression is unbuilt/unrun under the final verification
deferral; explicit in-process diagnostic and source access remain available.
Runtime command/dispatch/input/effect Debug output also now avoids recursively
printing URLs, command arguments and event payloads, with an authored runtime
transport sentinel test. Direct core event/engine-effect, tab-transfer, URL,
navigation, parsed-command and invocation/context Debug projections were also
made variant-only or bounded/redacted, with sentinel regressions for page URLs,
query strings, titles, command arguments and routing context. All changes and
tests remain unbuilt/unrun under the final verification deferral; application/
Qt dynamic diagnostic channels and the full lifecycle/fault campaign remain
open. See `docs/testing/versioned-browsing/VB-056.md`.

The subsequent core data-flow pass also replaced derived `Debug` on profile,
tab, window, application and journey state with bounded/redacted summaries;
redacted page-derived hint candidates/interactions/errors, completion values,
and cleaned-link rules/results, redacted binding commands/key sequences, and
hid structural-capture page/window IDs and context references from `Debug`;
cookie metadata site/backend dimensions and versioning event/effect IDs are
also now redacted. Captured-target/scope, branch-record and versioning-state
debug projections omit identity tokens; sentinel regressions were added.
These source/tests are likewise authored but not built or run during the
end-of-goal verification deferral. Remaining application/Qt diagnostics and
the full native lifecycle/fault campaign remain open. See
`docs/testing/versioned-browsing/VB-056.md`.

A further VB-056 exception audit found configuration errors displaying local
include paths and dynamic parser/validation text, while config/snapshot debug
could expose search URLs and override values. Error formatting now uses fixed
categories, config/override and application snapshot debug projections redact
values and paths, pending setting keys/values are suppressed, and theme
palette errors/configuration plus profile/context, permission/site-rule and
action-target projections redact local paths and user values. Sentinel
regressions and updated fixed-category expectations are authored but not run;
see `docs/testing/versioned-browsing/VB-056.md`.

The remaining read-only diagnostics no longer pass through Hyprland probe
errors, cached clean-link manifest/parser failures, or arbitrary action-audit
category strings. Clean-link diagnostics omit manifest prose and report a
constrained revision token/source class. Site-report, site-data-clear and
diagnostics-export status messages use fixed failure categories rather than
backend/filesystem details. Sentinel regressions are authored but unbuilt and
unrun under the user's verification deferral. The broader app/Qt diagnostic
and native lifecycle/fault campaign remains open; see
`docs/testing/versioned-browsing/VB-056.md`.

The application/storage boundary now gives profile activation/current-profile,
profile/context commands and context effects bounded `Debug` summaries.
Storage context members/descriptors/records suppress IDs, restore URLs, titles,
labels, profile names, workspace names and timestamps. Storage-root and profile
registry/lock/deletion errors use fixed `Debug`/`Display` categories while
retaining typed path/message fields for explicit in-process handling; root,
registry, profile-record and deletion projections redact local paths and user
identity. Sentinel regressions are authored but remain unbuilt/unrun during the
final verification deferral. Broader platform logging and native lifecycle/
fault coverage remain open; see `docs/testing/versioned-browsing/VB-056.md`.

Direct storage-manifest `Debug` output now summarizes window/page/warning
counts instead of exposing commit, logical-page/window IDs, URLs, selected
page, context, warnings or manifest prose. Commit summaries/details and delete
receipts redact durable IDs, capture timestamps, logical-page failure IDs and
environment fingerprints. Application commit-diff/list/info/delete/restore
events, planned restore pages/windows, policy previews, operation envelopes,
stage tickets and lifecycle effects now use redacted ID-free `Debug`
summaries; serialization and exact in-process values are unchanged. Focused
sentinel regressions remain unbuilt/unrun under the user's verification
deferral; broader lifecycle/fault coverage remains open.

Versioning status-bar messages now omit commit and operation IDs across queued,
durable, failed, detail and restore outcomes; explicit panel payloads and
addressed engine routes retain their IDs. Isolated archive-view metadata
`Debug` also redacts capture time and derived fidelity text. Sentinel
assertions are authored but unbuilt/unrun. Core command-error `Display` also
uses fixed messages for dynamic command/alias names and control characters,
closing an adjacent IPC/UI diagnostic path. This regression is likewise
authored but unbuilt/unrun. The broader application/Qt diagnostic and native
lifecycle/fault campaign remains open; see
`docs/testing/versioned-browsing/VB-056.md`.

VB-024 shutdown ownership now rechecks the exact secondary owner's retained
download maps immediately before deferred destruction. Requests arriving after
shutdown approval are canceled without opening UI but keep the completion
owner until Qt's terminal callback; that callback retries approved release.
Nested-owner discovery changes retry hidden parent release as well. An authored
QML regression covers retaining a hidden restore parent until its child closes
and its download completion entry is cleared; a source-boundary regression
asserts that late requests are recorded before cancellation and release happens
only after terminal-owner removal. These tests remain unrun; forced or abnormal
post-destruction callbacks and a genuine viewless request on Ferric's Qt Quick
profile remain unqualified. See `docs/testing/versioned-browsing/VB-024.md`.

VB-056 storage diagnostics now redact filesystem paths and OS error text from
`DownloadPathError` and `CrashMarkerError` `Debug`/`Display` projections while
preserving their typed fields for explicit handling. Sentinel regressions cover
all download error variants and crash-marker I/O; they are authored but remain
unbuilt/unrun during the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

The same audit hardened core URL/navigation error `Display` and direct
`DispatchError` formatting: caller-provided schemes, malformed input, command
text, argument labels and external URI recipients are omitted from diagnostics,
while typed variants still retain exact values for explicit handling. Sentinel
regressions are authored but unbuilt/unrun; platform logging and full lifecycle
qualification remain open. See `docs/testing/versioned-browsing/VB-056.md`.

Compositor and IPC-decoder errors now suppress backend/socket/invalid-payload
text from direct `Debug`/`Display`; the IPC error's typed diagnostic remains
available to the sanitized public response path. Sentinel regressions are
authored but unbuilt/unrun. `IpcRoute` debug output now reports selector kind
and presence bits, not runtime IDs/profile/context labels. Its regression is
also unrun. `ReduceError` debug/display now emits only fixed categories and
messages rather than runtime profile/window/tab IDs, with sentinel coverage
authored but unrun. `SessionError::Invalid` also uses fixed `Display` text
instead of exposing its public string payload; its regression remains unrun.
See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-026 archive-view continuation: Unix archive-view setup now
atomically renames the verified reserved `capture.bin` leaf to
`capture.mhtml` with no replacement, then verifies the destination against
the held descriptor before accepting it. A raced replacement is left intact
for recovery instead of being unlinked. An authored race-seam regression
preserves an outside hard-link sentinel. Non-Unix retains its path-based
transition; same-UID in-place mutation and malicious same-UID name races
remain outside the claim. Source/test are unbuilt/unrun under deferred
verification. See `docs/testing/versioned-browsing/VB-026.md`.

VB-024/VB-045 shutdown routing now guards the primary profile path as well as
restore-secondary windows. Ordinary post-approval requests are recorded under
their exact completion owner before exact-resolver cancellation and never open
a chooser; late Save Page requests are quarantined by request ID and staging
path before cancellation. QML helper and source-boundary regressions are
authored but unbuilt/unrun. Terminal callback delivery after process exit and
forced destruction remain unqualified; see VB-024.md and VB-045.md.

VB-056 profile-operation errors now map to fixed failure categories before
reaching Qt status text; registry and filesystem detail is no longer echoed
from profile worker failures. Successful list and explicit delete-preview
content is unchanged. Renderer callback errors for action selection, userscript
selection, selection copy, and caret editing also use fixed UI messages rather
than page-supplied exception text. External-editor preparation, launch,
read/wait, and write-back errors, selection-based process spawning, and
userscript action-completion failures use fixed categories instead of command
or OS diagnostics. Subprocess pipe-read errors are also fixed-category;
authorized edited content and bounded stderr are unchanged. Source-boundary
and sentinel regressions are authored but unbuilt and unrun; broader Qt
diagnostics and lifecycle/fault qualification remain open. See
`docs/testing/versioned-browsing/VB-056.md`.

Profile bootstrap admission, asynchronous setup failures, and degraded-ready
storage/configuration statuses now use fixed categories instead of formatting
local paths, OS errors, or parser text into the UI. Blocklist and clean-link
update status paths also omit downloaded identifiers, filesystem/parser
details, and untrusted revision strings; userscript install-queue failures also
use fixed QML/status categories instead of exposing manifest path/read/parse
diagnostics from queue admission or asynchronous inventory/install/remove/
enablement results. Source-boundary regressions are authored but unbuilt/unrun;
see
`docs/testing/versioned-browsing/VB-056.md`.

Shutdown flush and stale-session-checkpoint cleanup statuses also suppress
storage-worker/SQLite diagnostics while retaining the recovery-data warning.
The source-boundary regression is authored but unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

Durable-library session/recovery, switcher, profile-metadata, command-history,
permission, download and journey errors now use fixed status categories. The
session error panel retains its marker without raw diagnostics, and the
command-history failure latch stores a fixed message. Source-boundary coverage
is authored but unbuilt/unrun; see `docs/testing/versioned-browsing/VB-056.md`.

History, permission and download batch-completion statuses now also suppress
raw `StorageOperationError.message` content, retaining fixed operation-level
failure categories. The source-boundary regression is authored but unbuilt and
unrun. VB-056 remains in progress; see `docs/testing/versioned-browsing/VB-056.md`.

Configuration writer/reload, print-completion, site-rule decoding, and
configuration-editor statuses now similarly suppress filesystem, parser, and
backend/process diagnostics. Successful configuration results are unchanged;
a source-boundary regression is authored but unbuilt/unrun. See
`docs/testing/versioned-browsing/VB-056.md`.

External-process spawn and wait errors no longer retain OS diagnostics in the
operation state; fixed failure categories are used, while IPC still publishes
the closed status kind. Its source-boundary regression is unbuilt/unrun.

Context configuration and route errors now suppress parser/serialization,
filesystem, and caller-supplied route-ID details; successful route results are
unchanged. Its source-boundary regression is authored but unbuilt/unrun. See
`docs/testing/versioned-browsing/VB-056.md`.

Switcher action availability and execution status text now suppresses dynamic
action/command errors with fixed categories; its presentation-boundary
regression remains unbuilt/unrun.

Completed-download open/reveal failures now suppress stored destination paths,
filesystem errors, and file-URL conversion details, with a source-boundary
regression authored but unbuilt/unrun. See `docs/testing/versioned-browsing/VB-056.md`.

Site Doctor config/fix, site-status, and diagnostics-JSON failures now also use
fixed summaries instead of dynamic parser, write, or IPC error detail. Their
source-boundary regression is unbuilt/unrun; successful payloads are unchanged.

## Userscript, print, and journey-export error redaction — authored 2026-10-05

Page userscript manifest/read failures, worker admission, print/Save Page path
validation and private print staging/output failures now use fixed UI
categories instead of forwarding local paths, parser messages, or OS details.
Journey-export admission, serialization, preview and atomic-write failures
also suppress dynamic worker/serializer/filesystem text. Successful explicit
export payloads and output paths are unchanged. A source-boundary regression
covers both modules and rejects dynamic interpolation/error-string conversion;
it remains unbuilt and unrun under the verification deferral. VB-056 remains
in progress; see `docs/testing/versioned-browsing/VB-056.md`.

The configuration-query IPC projection now omits caller-supplied keys from
not-found/scope/registry failures and suppresses serde decode diagnostics for
the validated configuration. Successful query payloads are unchanged; the
source-boundary regression is authored but unbuilt/unrun.

Download admission/finalization now also discards raw OS/path and worker
diagnostics, including direct download-directory creation failures. Successful
user-selected download paths remain explicit. Its expanded source-boundary
regression is authored but unbuilt/unrun; see `docs/testing/versioned-browsing/VB-056.md`.

Opt-in `operations.query` stderr now redacts standalone Unix, Windows drive/UNC,
and `file:` paths in addition to secret-looking fields and URL query secrets;
the rest of a line containing a local path is suppressed to cover paths with
spaces. `operations.query` status projection now uses the same boundary while
retaining internal status unchanged for cancellation/terminal logic. The
regression is authored but unbuilt/unrun; the explicit bounded stderr retrieval
contract remains unchanged.

Userscript execution admission now suppresses manifest/parser, executable,
serialization, and process-start diagnostics before they can become UI or IPC
errors. Successful results and internal worker state are unchanged; source
regression remains unbuilt/unrun.

Ad-block compilation status no longer includes cache-list IDs or parser text,
and `PolicySnapshot` Debug now reports bounded counts/presence rather than
hosts, selectors, source IDs or diagnostics. Worker policy values are unchanged;
sentinel and source-boundary regressions are authored but unbuilt/unrun.

Another VB-056 pass replaced derived Debug output for staged download paths,
userscript manifests/inventory/page source/action results, and blocklist
metadata/cosmetic selectors with redacted bounded summaries. Serialized and
in-process values are unchanged. Sentinel regressions are authored but
unbuilt/unrun under the end-of-goal verification deferral; broader application
and Qt diagnostic sinks plus lifecycle/fault qualification remain open. See
`docs/testing/versioned-browsing/VB-056.md`.

Userscript terminal operation failures now also use fixed categories for
protocol, child-exit, wait, and pipe errors. Validated bounded success messages
and explicit bounded stderr remain unchanged. A source-boundary regression is
authored but unbuilt/unrun; see `docs/testing/versioned-browsing/VB-056.md`.

Search resolution errors now use fixed categories rather than echoing the
caller-supplied engine name or configuration/navigation parser details. Valid
resolution is unchanged; sentinel and source-boundary regressions are authored
but unbuilt/unrun. See `docs/testing/versioned-browsing/VB-056.md`.

File-only storage worker stage/archive handles and completion `Debug` output
now redact operation UUIDs as well as private paths/digests; typed routing data
is unchanged. A sentinel regression is authored but unbuilt/unrun. See
`docs/testing/versioned-browsing/VB-056.md`.

Storage journal-event `Debug` now reports only the closed event kind, not its
publication timestamp or operation UUID; persisted event routing is unchanged.
Its sentinel regression is authored but unbuilt/unrun.

Manifest policy and deletion-summary Debug output also redacts stable schema
and object-graph fingerprints. Serialization and confirmation semantics are
unchanged; sentinel regressions remain unbuilt/unrun.

An additional Qt transient-state pass removed derived Debug from switcher
candidates/results and caches, pending navigation/selection/action, editor and
download records, permission route keys, live-window records,
userscript-manager results, and generic worker poll values. These can contain
visited URLs/titles, search terms, opaque IDs, argv, local paths, script/page
data, process diagnostics and blocking evidence. Required debug projections
now suppress dynamic values or expose only bounded presence/count summaries
for settings, font, query-error, compositor-client and queued-action records.
Serialization, worker routing and in-process values are unchanged. Sentinel
regressions and a source-boundary guard against restoring derived Debug are
authored but unbuilt/unrun. The broader diagnostic-sink and native
lifecycle/fault campaign remains open; see
`docs/testing/versioned-browsing/VB-056.md`.

Process working-directory setup and editor process polling also suppress OS
error text that could contain local paths. A presentation source-boundary
regression is authored but unbuilt/unrun.

VB-026 live restart/quarantine and staging/viewer/unpublished-quarantine byte
admission scans now include nested files in their shared 100,000-entry global
traversal ceiling, limiting each child scan by the remaining budget. The
authored exact-boundary/over-boundary regression (including checked arithmetic
overflow) has not been built or run under the user's verification deferral.
This is an entry-count bound, not a profile-volume quota or same-UID race
guarantee; VB-026 and VB-091 remain in progress. See
`docs/testing/versioned-browsing/VB-026.md`.

VB-056 profile bootstrap no longer carries raw configuration, context,
storage, or worker-start diagnostics through its public result/error types.
The UI receives closed issue-presence flags and keeps its existing generic
status messages. Focused application regressions are authored but remain
unbuilt/unrun under the final verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

The userscript and external-action catalog failure statuses also no longer
display filesystem, manifest, configuration, or caller-supplied subject error
details. A source-boundary regression is authored but unbuilt/unrun; VB-056
remains open for the broader diagnostic and lifecycle/fault campaign. See
`docs/testing/versioned-browsing/VB-056.md`.

The shared Qt runtime bridge now passes only the explicitly safe runtime
`user_message` to UI callers rather than internal diagnostic context. Its
source-boundary regression is authored but unbuilt/unrun; this does not close
VB-056's broader diagnostic/lifecycle campaign.

Hint-triggered userscript failures also no longer echo manifest, executable,
or worker diagnostics into the status bar. A source-boundary regression is
authored but unbuilt/unrun; VB-056 remains open.

The `actions.query` IPC projection now substitutes fixed messages for
configured-action decoding and userscript catalog discovery failures. Its
source-boundary regression is authored but unbuilt/unrun; VB-056 remains open.

The shared `action-list` projection also suppresses invalid-subject,
configured-target, and userscript-discovery detail before UI or IPC response
construction. The source-boundary regression is authored but unbuilt/unrun.

UI action start/failure statuses now use fixed operation categories rather
than echoing dynamic external/userscript action IDs or error projections. The
typed audit path is unchanged. A source-boundary regression is authored but
unbuilt/unrun; VB-056 remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

Count-rejected configured keybindings also use a fixed reason rather than
echoing their command (which can contain a URL argument). Core sentinel and
Qt source-boundary regressions are authored but unbuilt/unrun; VB-056 remains
open. See `docs/testing/versioned-browsing/VB-056.md`.

The `action` command and page-userscript failure presentation also no longer
echo dynamic action/script names. Command-based userscript discovery, manifest,
executable and context-delivery errors are reduced to fixed categories at the
boundary. Source regressions are authored but unbuilt/unrun; VB-056 remains
open. See `docs/testing/versioned-browsing/VB-056.md`.

Built-in action mapping errors also suppress caller-supplied subject, verb and
argument text before returning to status or IPC consumers. Sentinel/source
regressions are authored but unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

Configured action-target catalog, missing-target, and subject-validation
errors now use fixed categories rather than echoing internal configuration
diagnostics or target aliases. Successful launch statuses retain the
user-visible target and operation ID needed for follow-up control. A
source-boundary regression is authored but unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

The disposable cookie-seed ownership follow-up now authors both destination-
and source-profile teardown while a native copy reply is pending. The
source-owner case requires one `OwnerLost` completion and permanently discards
its possibly mutated child. This extends no passing evidence or production
capability; the source-pinned native verification remains deferred. See
`docs/testing/versioned-browsing/engine-capability-spike.md`.

External-process stderr redaction now also suppresses cookie and authorization
values in HTTP-style header lines, `cookie=...` tokens, and URI query keys.
Any prefixed log text is retained only after the existing URL/path redaction.
This closes a diagnostic projection gap for explicitly requested operation
stderr and editor/userscript output. Sentinel coverage is authored but
unbuilt/unrun; see `docs/testing/versioned-browsing/VB-056.md`.

The VB-024 restore-allocation fixture also covers a constructed child that the
process-wide owner refuses to retain: it must be destroyed while source tab,
URL and window identity stay unchanged. This authored QML regression is
unbuilt/unrun; successful native multi-page restore and abnormal-destruction
callback behavior remain open. See `docs/testing/versioned-browsing/VB-024.md`.

Window/context command and IPC route/search/switcher failure diagnostics now
suppress caller-supplied option, context/profile, scope/action, entry-point,
route-ID, caret-direction and mode-entry text. Route add/remove status
summaries use fixed labels while explicit result payloads remain intact. The
source-boundary regression is authored but unbuilt/unrun; see
`docs/testing/versioned-browsing/VB-056.md`.

The hint privacy audit closed another caller-controlled diagnostic path:
malformed page candidate JSON, candidate/selection rejection, and hint
navigation or clean-link failures now return fixed status categories and IPC
error tokens. Parser errors no longer include serde details or candidate
counts, and core `HintError` debug/display omits page-derived counts and
positions while preserving typed values internally. Sentinel and
source-boundary regressions are authored but unbuilt/unrun. Broader
application/Qt sinks and lifecycle/fault qualification remain open; see
`docs/testing/versioned-browsing/VB-056.md`.

VB-045's remaining shutdown gap now has a feature-gated native journey: pause
the real archive object worker after Qt save completion, exercise normal
cancel-and-quit, require the worker to remain tracked while paused and the QML
capture owner/request to retire while that job remains pending, then resume it
and require shutdown to settle with no durable capture commit. The
test is registered in the archive campaign but remains unbuilt/unrun under the
verification deferral. This does not bound OS filesystem blocking or change
the production capture gate; see `docs/testing/versioned-browsing/VB-045.md`.

VB-056's Site Doctor `page.userscripts` projection no longer returns bounded
but dynamic userscript-discovery errors, which could still contain a profile
path or OS diagnostic. It now uses a fixed unavailable reason; sentinel and
source-boundary regressions are authored but unbuilt/unrun. Site-status page
titles also use the bounded bidi/control sanitizer instead of newline-only
replacement. Broader diagnostic and lifecycle/fault qualification remains open; see
`docs/testing/versioned-browsing/VB-056.md`.

The VB-026 storage race follow-up now uses safe `rustix` directory-relative
operations for explicit archive-view cleanup, post-publication staging
retirement and corrupt/orphan quarantine:
directory components are opened without following symlinks, batch creation,
rename and unlink are anchored to held directory descriptors, and Linux and
Apple quarantine renames atomically refuse an existing destination. Other
Unix targets now fail closed rather than use a check-then-rename fallback;
Redox and ESP-IDF also fail closed where `rustix` does not expose required
descriptor-relative calls.
The pinned `rustix 1.1.4` API inspection found Redox lacks `openat`, `mkdirat`,
and `unlinkat`, while ESP-IDF lacks `statat` and `unlinkat`; the wrappers now
return unsupported for those calls, and Redox is no longer claimed as an
atomic-rename target. This source-only portability correction is unbuilt and
untested.
The adjacent object-install path also creates fixed `object.tmp` and
`sidecar.tmp` leaves with exclusive descriptor-relative `openat`, links them
into a SHA-256 shard created/opened with `mkdirat`/`openat` under a pinned
objects-root descriptor, and removes them with `unlinkat`; its best-effort
temporary-file drop guard keeps the staging directory descriptor. Archive-view
staging now creates `capture.mhtml` and removes `capture.bin` relative to its
pinned operation directory. An authored helper regression ensures the shard
and exercises create/link/unlink after the source directory is renamed and its
old pathname becomes a symlink. All additions are unbuilt and unrun; leaf-name
swaps between syscalls and same-UID rewrites remain possible.
The native capture reader now also uses no-follow, nonblocking `openat` on its
fixed leaf after pinning the operation directory and checks descriptor type,
owner, mode, and link count before reading. Its helper regression reads a
candidate after the original parent path becomes a symlink. This avoids a
second pathname resolution between directory validation and file open; it
does not prove the opened directory is the inode first allocated to the
operation, nor stop same-UID leaf replacement. This source and regression are
unbuilt and unrun.
Committed-object verification, sidecar reads, archive materialization, and
same-digest reuse now open shard directories below a pinned objects root and
open regular leaves relative to the pinned shard after checking their
identity. The archive-view write target is also opened relative to its pinned
operation directory after identity validation. This narrows ancestor
redirection for object reads/writes but does not exclude same-UID leaf swaps
or same-inode mutation; these additions are unbuilt and unrun.
The workspace unsafe-code
denial was not relaxed. Authored, unbuilt regressions cover parent symlink
replacement, no-overwrite destination collision, and source-leaf swaps in
corrupt-reference and restart quarantine. Quarantine verifies the moved inode
and syncs a mismatched entry into its recoverable batch before refusing. This
narrows path redirection; it does not isolate Ferric from a same-UID process
that can relocate or rewrite profile-owned directories. The cleanup
stat-to-unlink race and same-inode rewrites remain. Keep VB-026 in progress and
include the new `dirfd.rs` test/dependency in the deferred final gate. See
`docs/testing/versioned-browsing/VB-026.md`.

The next VB-026 pass converted Unix versioning-store initialization and stage
reservation to pinned directory descriptors: initialization walks and verifies
the canonical state root, creates the profile tree relative to held parents,
and syncs those parents; operation and additional window-page staging use
exclusive descriptor-relative directory/file creation and sync before handing
Qt its fixed capture path. Reopening a staged leaf and converting it to an
archive-view target also stay relative to the pinned operation directory, and
directory opens compare the no-follow name identity with the opened descriptor.
Store initialization now also rejects Redox/ESP-IDF as `UnsupportedPlatform`
before creating profile state because required pinned `rustix` operations are
absent; filesystem-backed test modules are excluded there.
An outside-symlink staging-parent refusal regression is authored. All these
changes are unbuilt and unrun. They reduce ancestor redirection but do not
prevent a same-UID leaf replacement or a native path writer racing the later
capture. See `docs/testing/versioned-browsing/VB-026.md`.

Capture scanning now keeps the operation-directory descriptor for the whole
read and checks both the current path's directory identity and the capture
leaf identity through that held descriptor before accepting the stream. An
authored mid-stream parent-rename/symlink regression expects `CaptureChanged`
and confirms the outside sentinel is unchanged. It is unbuilt and unrun; the
consumer must discard already-streamed bytes on error, and same-inode writes
remain possible.

Archive-view conversion also rechecks the original capture leaf's descriptor-
relative identity against its retained open descriptor before unlinking it.
A new helper regression exercises leaf replacement under a pinned parent; it
is unbuilt and unrun, and POSIX still offers no unlink-by-inode primitive to
remove the final stat-to-unlink race.

Another VB-026 race pass now verifies that a newly linked object/sidecar path
names the exact open temporary inode, with the expected byte count and
two-link transition, before returning a receipt. Temporary cleanup also
compares the leaf to the original descriptor identity and preserves a
substitute instead of unlinking it. The authored object-link swap regression
cases cover replacement before linking and replacement after linking but
before retirement; they are unbuilt/unrun and require no receipt plus preserved
substitute bytes. This does not stop same-inode mutation or make cleanup unlink
conditional; verify content at durable publication and keep VB-026/VB-091 open.
See `docs/testing/versioned-browsing/VB-026.md`.

VB-056 also removes user-defined context/profile names from context-route and
context-switch status text. Labels remain in explicit routing actions and
structured command results, but shared status can cross an IPC error-result
boundary. Same-/cross-profile and already-open/separate-window fixed messages
have a source-boundary regression; this change remains unbuilt/unrun and does
not close the broader diagnostic campaign. See
`docs/testing/versioned-browsing/VB-056.md`.

Unsupported IPC method errors now return a fixed message rather than echoing
the client-provided method string. A source-boundary regression is authored
but unbuilt/unrun; the broader VB-056 privacy/fault campaign remains open.

Invalid command-context source errors likewise use a fixed category rather
than copying an untrusted source label into the IPC diagnostic context. The
source-boundary regression is authored but unbuilt/unrun.

IPC command failures also no longer forward shared UI status text for rejected
input-workflow commands, Site Doctor start/undo failures, or clipboard
navigation rejection. Malformed Site Doctor result JSON maps to fixed text
instead of exposing parser diagnostics. The source-boundary regression is
authored but unbuilt/unrun; continue the VB-056 diagnostic sink audit without
running verification until implementation is complete.

The `blocking.status` IPC projection now uses an `enabled`/`disabled` label
instead of forwarding the global status-bar text, which could carry an
unrelated page or operation diagnostic. Explicit blocking evidence fields are
unchanged. Its source-boundary regression is authored but unbuilt/unrun.

VB-045 now has a complementary native two-page partial-capture journey: save
the first page, cancel the exact second-page Qt Save Page request, then require
the reopened manifest to retain the first page's archive/sidecar and record a
closed cancellation for page two. The test probe resolves capture-page index
from the exact QML save owner's logical tab ID, not whichever view is active.
This source/fixture addition is unbuilt/unrun under the requested end-of-goal
verification deferral.

The same indexed journey now includes a second-page navigation race after its
collector and page-save claim. The expected durable partial result keeps page
one complete and marks page two `document-changed` without reusing its
pre-navigation sidecar. This fixture is also authored but unbuilt/unrun.

VB-045 now also authors the later-page archive-copy storage-full ordering:
after the first page's archive and sidecar are installed, the feature-gated
fault hook injects one `ENOSPC` during the second page's object copy. The
reopened-manifest assertions require page one to stay complete and only page
two to record `storage-full` without an object, sidecar, or interval. Registered
in the native archive journey list, this is unbuilt/unrun under the requested
verification deferral and does not simulate a physically exhausted filesystem.

The later-page matrix also now includes an authored timeout route: after page
one is installed, the native probe waits for page two's exact save owner and
expires that operation through the production retirement callback. Reopened
manifest assertions require only page two to carry `timeout`; this is a
deterministic trigger, not a measured 90-second deadline. It is registered but
unbuilt/unrun under the deferred verification campaign.

The later-page failure matrix also has an authored native-view-loss journey.
It closes the second logical tab only after checking the exact active capture
owner and expects page one to remain complete while page two alone records
`native-view-lost`. The fixture is registered but unbuilt/unrun; close and Qt
callback ordering still require the final campaign.

VB-026 descriptor-relative helpers now enforce their own one-leaf-name
invariant: empty, dot/dot-dot, slash-containing, and embedded-NUL names refuse
before any filesystem syscall. A source regression tries each form through
read/open/create and link/mkdir/unlink/rename helpers, then checks the source
file and outside sentinel were preserved. This hardening and regression are
unbuilt/unrun under the end-of-goal verification deferral. Same-UID races and
the broader VB-091 fault campaign remain open; see
`docs/testing/versioned-browsing/VB-026.md`.

VB-024's admitted-restore-plan regression now also mutates a page's public
`pinned`, `muted`, and `zoom_hundredths` metadata independently after planning,
and changes the navigation-order vector. Execution must reject each with
`UnsafeRestoreDescriptor` and leave the source-window tab list unchanged.
These supplement the safe-URL substitution case and are authored but
unbuilt/unrun under the deferred final-verification campaign. See
`docs/testing/versioned-browsing/VB-024.md`.

VB-044 now also tests an exact one-use Save Page path paired with a wrong
public format. The route must cancel and retire that operation/path without
calling the Rust admission bridge; matching path alone is not a complete
permit. This QML regression is authored but unbuilt/unrun. Viewless
ordinary-download durable-ledger proof and VB-057 release qualification stay
open; see `docs/testing/versioned-browsing/VB-044.md`.

VB-056's runtime error-chain sentinel regression was stale: it required an
external-navigation URL to remain in diagnostic context even though
`DispatchError::Display` now emits a fixed category. `RuntimeError` now drops
its consumed typed source so caller-supplied URLs/command text cannot be
recovered through `Error::source` downcasting; its code and fixed category
remain. The regression requires no source and no URL/query sentinel. It is
authored but unbuilt/unrun.

2026-10-05 VB-044 public-header follow-up: installed Qt 6.11.2's
`qquickwebengineprofile.h` confirms there is no public accessor to the wrapped
`QWebEngineProfile`; the core `QWebEnginePage` download API cannot target
Ferric's exact custom Quick profile through public APIs. Qt's public extension
manager is not a qualified alternative: Qt documents MV3 loading and
off-the-record exclusion, but not support/semantics for `chrome.downloads` as
a viewless profile request. This remains an evidence/architecture blocker; do
not use private Qt APIs or claim the native viewless-to-ledger journey is
closed. See `docs/testing/versioned-browsing/VB-044.md` and `VB-024.md`.

2026-10-05 candidate same-profile stimulus authored, not run: a dedicated
native probe loads a disposable MV3 extension through the public Qt Quick
profile extension manager, asks its service worker to call
`chrome.downloads.download()`, and requires a real download signal with a null
public core-page pointer. The QML handler routes the callback through the
production `ProfileDownloadRouting` exact-profile-owner helper and retains the
request through terminal delivery without reading `download.view`. Run only
during final verification with `cargo xtask test versioning-viewless-download`.
It is a test-only candidate (not production extension functionality or a
durable-ledger journey); keep the blocker and production capture gate until the
result is known and any production-equivalent lifecycle is evidenced.

VB-056 diagnostics export now separates ordinary report reads from explicit
export authorization. The report shown to the user is bounded at the export
limit; oversized/error snapshots cannot authorize a save. On Save, the native
bridge requires byte-for-byte equality with the exact read-only text currently
shown, and later report reads clear any pending authorization. Export consumes
that reviewed payload once. This closes the hidden-full-report/truncated-preview
mismatch and prevents background engine-status reads from arming an export.
Sentinel and UI source-boundary regressions are authored but unbuilt/unrun;
see `docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-056 renderer-result follow-up: selection-search completion now
maps any renderer-result `error` field to fixed status text rather than
reflecting the value into status/operation projections. The presentation
boundary regression is authored but unbuilt/unrun. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-055 key-store research: Linux Secret Service and Rust's keyring
API can support a profile-scoped external secret, but the Secret Service
standard does not mandate application ACLs or even master-password locking;
lock prompts are implementation-specific. This may address an offline,
detached-sidecar reader without the source keyring, but not a same-user process
with access to an unlocked service. No dependency/key path has been introduced:
capture and restore currently compute fingerprints inside ApplicationWorld,
and the key lifecycle plus trust-boundary redesign needs explicit threat-model
acceptance. Media seek stays release-gated; details and primary references are
in `docs/testing/versioned-browsing/VB-055.md`.

2026-10-05 VB-056 external-editor follow-up: external editor stderr is now
discarded at process launch rather than retained or passed through QML; QML
returns only a fixed failure marker. Configuration-editor errors/status no
longer disclose local paths, exit details, or stderr, and its accepted result
omits the config path. Source-boundary regressions are authored but unbuilt and
unrun. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-024 native routing follow-up: the real-profile download fixture
now configures an exact same-profile Save Page capture path on an owned child
window and routes the Qt request and terminal callback through the bounded
owner-tree path matcher without reading `download.view` on that branch. The
ordinary per-view case remains the control. This does not demonstrate a
genuinely viewless callback or Rust durable-ledger completion; both remain
unqualified and the separate VB-044 blocker is unchanged. The fixture is
authored but unbuilt/unrun. A source-boundary regression also requires the
production exact-save block to resolve path/owner before any view-getter read;
it too remains unrun. Details: `docs/testing/versioned-browsing/VB-024.md`.

2026-10-05 VB-056 URL diagnostic follow-up: Qt/Rust canonical-origin mismatch
errors now use a closed category and fixed display text instead of embedding
either observed origin in status diagnostics. Sentinel and source-boundary
regressions are authored but unbuilt/unrun under the final-verification
deferral. Other Qt/Chromium diagnostic channels and the wider lifecycle/fault
campaign remain open; see `docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-029 documentation reconciliation: `structural-release.md` now
distinguishes the campaign recorded for its then-current source snapshot from
continuation edits that still require end-of-goal verification. The structural
boundary remains a draft and does not approve release; restore/storage
acceptance and dependent native gates remain open. See `VB-029.md`.

2026-10-05 VB-056 async profile/context status follow-up: profile lookup/open
and context creation completion statuses no longer echo user-defined profile
names, labels, or context names; context creation failure text is fixed. The
explicit profile-list result remains intact. A source-boundary regression is
authored but unbuilt/unrun. Hyprland workspace route/move and browser-client
query failures likewise suppress dynamic socket/OS diagnostics. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-056 configuration-write status follow-up: queued configuration
export/default-write and asynchronous writer-completion status messages no
longer echo the selected local path. Successful completion reports the byte
count. `config-check` and `config-reload` results now report a `configured-file`
source category rather than the private path, while their worker requests keep
the exact configured path. The `theme-reload` palette preserves its fields but
replaces the local theme path in both palette and provider details with a fixed
redaction marker. Explicit export/default-write command results still return
the caller-selected path. Source-boundary regressions are authored but
unbuilt/unrun under the final verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-05 VB-056 profile-deletion status follow-up: successful profile
deletion status no longer interpolates the user-defined profile name, while
still distinguishing cleanup-pending from completed cleanup and preserving the
QtWebEngine-storage boundary. A source-boundary regression is authored but
unbuilt/unrun under the final verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 continuation: unsupported interactive open/history-clear
options, invalid binding modes, unknown get options, action subjects, action
IDs, and action argument keys no longer echo caller values through parser
errors or public IPC diagnostic context. URI/query-secret sentinels cover these
paths in authored tests. These additions are unbuilt/unrun under the user's
final-verification deferral; VB-056 remains in progress. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-026 continuation: exact post-publication staging cleanup now
routes its operation-directory and staging-parent sync calls through a narrow
injectable I/O seam; production still invokes `File::sync_all`. An authored
object-backed publication regression makes each sync call return an
EIO-shaped error in turn and requires the durable commit to remain published,
cleanup to report incomplete, and restart recovery to reconcile the exact
remaining stage state. This tests propagation at the sync-call boundary, not a
kernel-generated fsync failure or power loss. The source/regression are
unbuilt and unrun per the end-of-goal verification deferral; VB-026/VB-091
remain in progress. See `docs/testing/versioned-browsing/VB-026.md`.

The same VB-026 pass adds a test-only sync callback to immutable-object
installation. After linking the content-addressed object, separate injected
EIO-shaped returns from staging-directory and object-shard sync calls must
withhold the object receipt; restart collection must account for the staging
operation and orphan object. Production still invokes real `File::sync_all`.
This is deterministic callback error propagation, not kernel fsync/power-loss
evidence; the source and regression remain unbuilt/unrun. See
`docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-045 continuation: added an application/storage regression that
installs a bounded archive object, corrupts one byte without changing its
length, and attempts publication. The storage worker must reject the digest,
the failed-publication route must settle reservation abort, and the profile
must retain zero visible commits. The application operation stays
`OutcomeUnknown` after publication began, as required by the conservative
callback contract. This regression is authored but unbuilt/unrun under the
end-of-goal verification deferral; physical corruption/crash recovery, native
failure cases and VB-057 release qualification remain open. Production archive
capture stays disabled. See `docs/testing/versioned-browsing/VB-045.md`.

2026-10-06 VB-056 privacy continuation: the normal-profile context-capture
test now includes a populated private-profile window in the saved membership,
with URL/query/title sentinels. It requires the member to be `WrongProfile`,
the normal manifest to contain only the valid source window, and no private
sentinel in serialized output. This is a targeted exclusion regression, not
arbitrary branch-to-window mapping or lifecycle qualification. It is
unbuilt/unrun under the final-verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-044 continuation: the earlier standalone public MV3 extension
probe is now wired as a prerequisite for a second ignored native journey using
the actual Ferric `Main.qml`, exact normal `QQuickWebEngineProfile`, production
download handler and disposable durable profile database. The journey checks
the completed ledger row, exact source URL and byte count, completion time,
download destination and installed fixture bytes. `cargo xtask test archive`
will run this integrated leg only when the standalone null-page/owner-routing
probe succeeds; unavailable Qt extension support is reported as a skip. All
new native code and runner wiring remain unbuilt/unrun under the end-of-goal
verification deferral. VB-044 remains blocked and the shipping archive gate
stays disabled pending the native evidence and VB-057 release qualification.
See `docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 VB-044 test-strengthening follow-up: the integrated journey now
attaches a test-only native observer to that exact production
`QQuickWebEngineProfile` before triggering the MV3 popup. It requires exactly
one matching non-SavePage request and `QWebEngineDownloadRequest::page() ==
nullptr`, then the existing production QML completion/ledger assertions must
pass for the same fixture URL and bytes. This removes reliance on combining a
null-page observation from a separate profile with a ledger result from Ferric.
The updated observer bridge and journey are unbuilt/unrun under the end-of-goal
verification deferral; VB-044 remains blocked pending execution and VB-057.
The extension worker now accepts only the two explicit loopback fixture routes
used by the standalone and integrated legs; this wiring correction is also
unverified.

2026-10-06 VB-024 orderly download-owner close regression: the restore-owner
QML test now models `isFinished == true` before the profile's terminal callback
arrives, and installs a throwing `view` getter on the exact retained request.
The approved child must remain retained until that identity is removed, then
release normally. This exercises the supported orderly-close invariant; it
does not claim callbacks after forced or abnormal destruction. The fixture is
unbuilt/unrun under the final-verification deferral. See
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-026 continuation: bounded capture streaming now snapshots Unix
mtime/ctime before reading and compares them after the final descriptor-relative
identity/size check. An authored regression rewrites a 64-KiB capture in place
after its first chunk, keeping the same pathname, inode and length; the scan
must return `CaptureChanged`. The fixture now waits up to two seconds for an
actual mtime/ctime change instead of assuming a fixed 2 ms delay crosses the
filesystem's timestamp-resolution boundary. This detects ordinary concurrent
rewrites when the filesystem reports a timestamp change, not a same-UID writer
lock. The source and regression remain unbuilt/unrun under end-of-goal
verification deferral. See `docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 cookie writer-route audit: pinned Chromium inspection confirms
ordinary CookieManager sets, RestrictedCookieManager sets and HTTP response
Set-Cookie handling converge on the CookieStore/CookieMonster setter, while
delete/clear/session cleanup use separate mutation methods. Therefore the
research seed overlay's CookieManager-local busy bit is not an exclusive child
writer lease; a real interlock must cover every mutation at the owning store
sequence or rely on a fully unpublished, writer-inaccessible child with native
evidence. The latter remains only a construction candidate: existing
page-free-child fixtures do not prove all internal writers absent or
durable-publication status. Updated `engine-capability-spike.md` and VB-075;
production cookie flags remain disabled and VB-076 stays blocked.

2026-10-06 VB-024 owner-lifetime continuation: popup windows are top-level
objects at the process root but can retain callbacks into the exact restore
window's UI bridge. Popup creation now records its opener host. An approved
restore window cannot release while one of its exact popups remains registered;
popup destruction unregisters first, then retries release. Malformed or
over-limit popup registration fails closed. QML and source-boundary regressions
are authored but unbuilt/unrun under the verification deferral. This preserves
the owner through orderly popup teardown, not forced process destruction. See
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-056 privacy continuation: the tab-suspension and tab-discard
refusal reasons flow into shared status text, so their producers are now
covered by a source-boundary assertion requiring fixed string-literal reasons
only. This excludes tab URL/title/page diagnostics from those refusal messages;
it does not qualify the lifecycle operations or all status sinks. The
regression is authored but unbuilt/unrun under the end-of-goal verification
deferral. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-024 popup-owner continuation: production restore-window popup
ownership detection now calls a shared QML predicate that keeps the owner
alive for exact opener-host, opener-UI, and popup-permission-UI registry
matches. Malformed/oversized registries and throwing property access fail
closed. The owner-retention regression now uses the real registry predicate
and verifies release only after popup unregister; additional malformed-state
cases are authored. The source and QML tests are unbuilt/unrun under the
end-of-goal verification deferral. Orderly popup teardown only; forced
destruction remains unqualified. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-026 bounded-query continuation: profile-wide GC, restart
recovery, and commit-deletion preview now preflight raw commit-object link rows
before DISTINCT-root and correlated-sharing queries. Publication also reserves
capacity for each incoming link row, not just each distinct object hash. This
prevents normal writes from creating duplicate-heavy profiles whose link
queries exceed the intended logical scan ceiling; damaged over-limit profiles
refuse before file or metadata mutation. An authored restart regression uses a
real orphan stage with repeated roots and requires refusal before quarantine
creation or stage movement. Focused admission/GC/recovery tests are authored
but unbuilt/unrun. The ceiling is for logical rows, not SQLite b-tree page
visits, physical disk use, same-UID races, or filesystem faults. See
`docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-056 status-projection continuation: the native stash adapter's
refusal and uncertain-outcome messages are fixed, but had no regression
covering its closed `StashError` variants. Added a test enumerating every
variant and requiring exact fixed copy for refusal plus the fixed uncertain
message, including nested scope/eligibility/operation/preview/consent errors.
No runtime behavior changed. The regression and source are unbuilt/unrun under
the end-of-goal verification deferral; broader diagnostic, IPC, logging and
lifecycle qualification remain open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 IPC status continuation: permission-list/reset,
site-data-clear, and command-action failure paths no longer return the global
status-bar string as an IPC error; each emits fixed operation-specific copy.
Malformed typed command argument JSON also suppresses serde details, and
configuration-reload rejection now returns fixed copy rather than the
shared status string. Successful payloads and ordinary UI statuses are
unchanged. Added a source-boundary regression for these producers. Changes are
unbuilt/unrun under deferred verification, and the broader IPC/logging/status
campaign remains open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-024 bounded popup continuation: popup ownership scans used a
64-entry bound but runtime registration did not enforce it, allowing an
unbounded process-wide registry that could permanently block approved-owner
release. New popup admission now checks the shared 64-entry limit before
allocation; registration rechecks it, refuses malformed state, and defensively
destroys an unopened popup if registration fails. Unregistration is now
malformed-safe. QML coverage exercises empty, invalid, throwing-getter,
below-limit, and at-limit cases, while a source-boundary regression requires
both production handlers to preflight before allocation and successful
registration before `openIn`. The implementation and tests are unbuilt and
unrun under the deferred final verification; orderly close only and the
viewless-download blocker remain. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-045 partial-publication continuation: strengthened the
application-level partial-window authorization journey so page one is lost,
page two installs successfully, and a later staged page records a fixed
timeout. The final publication assertions now retain only page two's object
and interval, preserve both page-local failure reasons, and refuse logical-ID
rebinding for failed pages. A fresh read of the durable profile manifest
asserts those same records after close. This complements (not replaces) the
authored native later-page ENOSPC journey. The updated application test is
unbuilt and unrun under final-verification deferral; production archive
capture remains disabled. See `docs/testing/versioned-browsing/VB-045.md`.

2026-10-06 VB-056 runtime-setting continuation: interactive set/unset failures
previously copied command errors into shared status and the Qt
`runtime_setting_error` property. Both now use fixed details-suppressed copy;
successful behavior is unchanged. Added a source-boundary regression for both
producers. Source and test remain unbuilt/unrun under end-of-goal verification
deferral; broader diagnostic and lifecycle/fault qualification remain open.
See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-045 pre-save identity continuation: after asynchronous sidecar
collection, the native adapter now revalidates that the exact captured view
still owns the logical tab and that its URL still matches the captured
document, before accepting sidecar output or claiming Qt's auto-starting Save
Page request. A changed document is recorded as `document-changed`; a missing,
replaced, empty, or throwing view is `native-view-lost`. If `view.save()` itself
throws, the same recheck preserves that classification; only a still-current
view maps to `invalid-archive`. Added QML helper and source-order/mapping
regressions. Successful archive completion also now routes an unavailable or
replaced exact view as `native-view-lost` rather than conflating it with
`document-changed`, and catches failures while resolving the live view. The
transient request-object match stays outside the strict serialized Rust facts
DTO. Source and tests remain unbuilt/unrun under deferred final verification;
production archive capture remains disabled. See
`docs/testing/versioned-browsing/VB-045.md`.

2026-10-06 VB-056 diagnostic continuation: the blocklist updater's HTTP 304
status previously included its configured cache/list ID, and the runtime
copied that status into shared UI state. It now emits fixed `Blocklist
unchanged` text while retaining the ID only for internal update routing. Added
a source-boundary regression. The source and test are unbuilt/unrun under
final-verification deferral; broader diagnostic and lifecycle/fault
qualification remains open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-026 portability audit: the archive-view transition keeps a
legacy path-based `cfg(not(unix))` branch, but `VersioningObjectStore::open`
returns `UnsupportedPlatform` for all non-Unix targets before mutation, so
that branch is not a supported runtime path. Corrected VB-026 evidence to say
this explicitly and require an identity-bound no-replace transition before
any future non-Unix store port. The Unix implementation and its regression
remain unbuilt/unrun under the verification deferral; VB-026/VB-091 stay open.

2026-10-06 VB-056 diagnostic continuation: asynchronous network-policy reload
failures no longer interpolate worker error text into shared UI status; they
use fixed details-suppressed copy to prevent local paths or parser/engine text
from escaping. A source-boundary regression was added. Source and test remain
unbuilt/unrun under final-verification deferral; broader diagnostic and
lifecycle/fault qualification remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 network-policy status correctness continuation: a requested
reload could report success even when its worker failed, and could overwrite
the compiler's fixed failure status. Completion now reports reload success
only for an error-free compiled snapshot, preserving the fixed failure result
otherwise. The regression covers this outcome precedence together with worker
diagnostic suppression. Source and test remain unbuilt/unrun under
final-verification deferral; the broader diagnostic and lifecycle/fault
campaign remains open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-026 portability hardening: the non-Unix path-based archive-view
rename/delete fallback has been removed. `StagedCapture::use_archive_view_path`
now returns `UnsupportedPlatform` before filesystem mutation on non-Unix,
Redox, and ESP-IDF targets, even if reached outside the store-open guard. Added
a target-gated regression for direct refusal and no operation-directory
creation. Source and regression remain unbuilt/unrun under final-verification
deferral; the supported Unix identity-bound transition and wider storage fault
qualification remain open. See `docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-026 portability hardening: capture reservation, extra window-page
reservation, sidecar installation, and immutable-object installation now
independently enforce `UnsupportedPlatform` before filesystem access on
non-Unix, Redox, and ESP-IDF targets, in addition to the existing store-open
and lower-level private-directory refusals. The supported Unix
identity-bound implementations are unchanged. Added a target-gated
no-mutation regression using direct test-only store/stage construction. Source
and test remain unbuilt/unrun under final-verification deferral; this is not a
non-Unix port or broader fault qualification. See
`docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-056 diagnostic continuation: successful keyboard-binding
feedback in learning mode previously echoed the full configured command,
including arguments that could contain URLs or search text. It now reports
only the registry-expanded command name, preserving dispatch behavior and
matching the help-map contract. Added a source-boundary regression. Source and
test remain unbuilt/unrun under final-verification deferral; broader Qt
diagnostic and lifecycle/fault qualification remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-026 sync-failure continuation: archive-view materialization now
uses an injectable directory-sync callback at the atomic staging-name rename
boundary; production still uses `File::sync_all`. Added a regression that
injects an EIO-shaped result and requires error return plus cleanup of the
incomplete viewer directory. This is same-process injected-error coverage,
not process-kill or power-loss durability evidence. Source and test are
unbuilt/unrun under the final-verification deferral. See
`docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-026 cleanup durability continuation: an idempotent archive-view
cleanup retry now syncs the pinned `views/` directory when the operation
directory is already absent, completing a prior removal whose parent sync
may have failed. Added an injected-error regression for final-sync failure and
retry. It is unbuilt/unrun and does not model an actual kernel fsync failure
or power loss. See `docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-056 hint-status privacy continuation: hint progress statuses no
longer echo the configured rapid target, typed prefix/text filter, page-derived
candidate counts, external target name, userscript name, or userscript action
ID. Fixed summaries replace those projections while the explicit structured
hint/action response is unchanged. Added a source-boundary regression. Source
and test remain unbuilt/unrun under the final-verification deferral; broader
Qt diagnostic and lifecycle/fault qualification remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 profile-list status continuation: async `profile-list`
completion no longer formats the serialized profile catalog into shared
status. It uses fixed `Profiles available`/`No durable profiles` summaries;
the explicit profile-list property remains unchanged. Extended the existing
source-boundary regression. Source and test remain unbuilt/unrun under
final-verification deferral; broader Qt diagnostic and lifecycle/fault
qualification remains open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 selection-search status continuation: request and completion
status no longer echo the configured search-engine name. The explicit
structured request keeps its engine field. Added a source-boundary regression;
source and test remain unbuilt/unrun under final-verification deferral.
Broader Qt diagnostic and lifecycle/fault qualification remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 QML continuation: desktop-notification block/show/revoke
status no longer includes the page origin, and the secondary-window
unsupported-action fallback no longer includes the action identifier. These
now use fixed summaries while exact values remain in their explicit routing
paths. Extended source-boundary assertions to cover successful presenter and
Qt-fallback notification paths. Source and tests are unbuilt/unrun under
final-verification deferral; broader Qt diagnostic and lifecycle/fault
qualification remains open. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 final-gate continuation: `cargo xtask check --locked --offline`
compiled after correcting a duplicate `std::fmt` import and native-journey
array length. The QML suite passed 208 tests with eight capability-gated
skips. The next native viewless-download probe crashed in QtWebEngine 6.11.2
inside `QWebEngineExtensionManager::setExtensionEnabled`; its coredump remains
in QtWebEngine frames. A Qt 6.11 skip guard was added to the disposable probe,
not to production capabilities. The standalone viewless-download witness and
Ferric ledger journey remain unqualified. Rerun the workspace gate after the
guard before reconciling or completing the goal; see
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 integrated final verification: reran `cargo xtask check --locked
--offline` after the Qt 6.11 extension-probe guard; it completed successfully
(exit 0). QML passed 208/208 with eight capability-gated skips. Native page
state, download ownership, Wayland archive-isolation, and the full real-window
archive capture/restore/lifecycle matrix passed. The durable viewless-download
journey was skipped because extension activation is unsafe on this runtime;
cookie/HTTPS observation passed for Secure and HttpOnly only, while SameSite
and Partitioned identity remain unqualified. The 37-input release artifact
manifest and 12-dependency license policy passed. Locked workspace unit and
doc tests passed, including the previously stale Qt and runtime fixtures.
`validate_pack.py --summary` reports 44 verified, 11 in-progress, 36 blocked,
and 5 not-started tasks; `--next` reports no dependency/capability-ready task.
These results supersede the earlier “rerun the workspace gate” instruction,
but do not close slices whose native/runtime acceptance is still unavailable.
Production archive capture/viewing and cookie identity capabilities remain
disabled or unqualified as recorded in their slice evidence.

2026-10-06 VB-044 alternate-runtime recheck: an isolated official Qt 6.10.3
runtime reproduced the extension-activation SIGSEGV seen on installed Qt
6.11.2. `QWebEngineExtensionManager::loadExtension` completed with the
extension disabled; the process crashed during/after enabling and before the
popup. The core points into QtWebEngineCore/V8 and has no Ferric frame.
Replacing the MV3 worker with direct popup download and then with a trivial
popup did not avoid the crash; stopping after load and before enabling did.
The experiment is not proof of the precise Qt defect, but shows the failure is
not specific to Ferric download routing or the worker script. The disposable
probe guard and runner now report this as an unsafe activation skip for Qt
6.10.3–6.11. Production capabilities remain disabled/unqualified; see
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 final gate after the cross-runtime guard: `cargo xtask check
--locked --offline` completed with exit 0. QML passed 208 tests with eight
capability-gated skips; the page-state/download probes and native
archive/isolation/capture/restore/lifecycle journeys passed. The extension
activation witness and dependent durable viewless-download journey were
skipped with the new explicit reason. Cookie/HTTPS boundaries, the 37-input
release artifact manifest, the 12-dependency license policy, locked workspace
tests, and doc tests passed. This is the final-gate result for the current
tree; it does not qualify SameSite/Partitioned cookie identity, viewless
download ownership, WebRTC-absence admission, or production archive enablement.

2026-10-06 VB-056 status-sink continuation: the site-blocker toggle previously
echoed the current page host in shared status when enabling or disabling a
bypass. The policy still updates the exact host, while status now uses fixed
summaries; a source-boundary regression rejects the former interpolation.
This follows the final gate and remains unbuilt/unrun under the end-of-goal
verification deferral. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 status-sink continuation: the interactive `site-data-clear`
preview previously serialized the active origin and category scope into
shared status. It now emits a fixed confirmation prompt while the dedicated
typed command path preserves its structured preview result. Added a
source-boundary regression for both projections. This follows the final gate
and is unbuilt/unrun under the verification deferral; see
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 status-sink continuation: confirmation of an external URI
previously repeated its caller-provided scheme in shared status. Dispatch
still receives the exact URI, while feedback now uses fixed copy; added a
source-boundary regression for both behaviors. This follows the final gate
and remains unbuilt/unrun under the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 IPC continuation: stable command encoder/decoder failures
now use fixed messages instead of echoing caller-supplied unknown options,
unsupported route-entry-point tokens, unexpected history arguments, invalid
send-subject values, malformed nested JSON parser detail, or unsupported
command names. Valid command payloads remain unchanged. Added sentinel
coverage across affected IPC paths; it is unbuilt/unrun under the verification
deferral. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 IPC handshake continuation: malformed typed hello parameters
now return fixed protocol copy instead of Serde's error string, which may
include a caller-supplied invalid value. Added a sentinel response regression;
it remains unbuilt/unrun under verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 IPC userscript continuation: userscript-action failures no
longer forward registered-manifest discovery errors into public IPC detail,
which could expose local file/OS diagnostics or manifest values. They return
fixed details-suppressed copy, with a source-boundary regression. Unbuilt/
unrun under verification deferral; see `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 IPC permission continuation: permission-origin query
normalization failures now map to fixed error copy instead of exposing the
underlying storage error string. Shared IPC query errors no longer retain
arbitrary diagnostic strings and project category-fixed text at both response
and config-command status sinks. Event-subscription startup failures also use
fixed response copy instead of forwarding thread/OS diagnostics. Added
source-boundary and unit regressions. They are unbuilt/unrun under verification
deferral; see
`docs/testing/versioned-browsing/VB-056.md`.

The scoped logging audit across Qt/application/IPC/storage found no production
Rust print/tracing/log calls. The only production Qt logging projection is the
fixed QML warning summary with a warning count; standard-output markers and
skip notices in the inspected crates are test-only. This is a source audit,
not runtime log qualification.

2026-10-06 VB-056 hint-parser continuation: the low-level page-hint candidate
parser still echoed an unknown page-supplied kind when invoked directly, even
though UI callers projected a fixed error. It now returns a fixed unsupported-
kind message, with a sentinel unit regression and source-boundary assertion.
Unbuilt/unrun under the verification deferral; see
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 userscript-protocol continuation: result parser errors now
use fixed categories instead of reflecting JSON parser detail or unlisted
process-supplied result/command names. Added sentinel unit and source-boundary
regressions; unbuilt/unrun under the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 browser-status continuation: permission, session, navigation,
clean-link, tab-transfer/lifecycle, and binding-parse failure statuses now use
fixed details-suppressed text instead of interpolating errors that can carry
origins, paths, parser output, or backend details. Download/permission/history/
journey write scheduling failures also use fixed status text. Added Qt
source-boundary regressions for the fixed categories and removed templates.
This continuation is unbuilt/unrun under the verification deferral; broad
diagnostic-sink and native lifecycle/fault qualification remains open. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 navigation diagnostic continuation: Chromium-provided
navigation failure prose was previously control-sanitized and truncated, but
could still contain page URLs or local/system details. The UI now retains only
the normalized failure category and safe URL context, with fixed explanatory
copy in the dialog and no dynamic detail in status. Added a source-boundary
regression; it remains unbuilt/unrun under the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 compositor status continuation: the direct workspace
route/move and browser-client query methods now use fixed details-suppressed
status copy for adapter availability and worker submission failures, matching
the config-runtime completion path. Extended the source-boundary regression to
both modules. It remains unbuilt/unrun under the verification deferral. See
`docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-024 continuation: authored an ignored native two-page committed-
geometry journey. It captures the selected page's nested scroll sidecar in a
two-page commit, checks restored structure and the inactive page's lazy flag,
then selects the sibling and awaits its safe GET. Added bounded native state
projection for lazy flags and a test-only adapter tab selector. Source changes
and journey remain unbuilt/unrun under the verification deferral; VB-024 and
production capture remain open. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 final verification continuation supersedes the preceding NOT RUN
note for the native two-page committed-geometry journey. The focused native
journey passed via
`FERRIC_NATIVE_ARCHIVE_JOURNEY=native_qt_committed_multi_page_geometry_keeps_inactive_restore_lazy cargo xtask test archive`;
the complete `cargo xtask check --locked --offline` also passed on this tree
(208 QML passed, eight capability-gated skips, native archive/capture/restore/
lifecycle matrix passed, workspace tests and doc tests passed). It proves the
selected page restores committed geometry, the inactive sibling stays lazy,
and selecting it triggers its safe GET and retires the lazy reservation. The
gate required adding the Rust declaration for the native selector helper and
correcting a stale source-contract assertion discovered by workspace tests.
This does not qualify Qt viewless-download delivery, exact cookie identity,
forced-destruction callbacks, or current-window replacement. VB-024 remains
in progress and production capture remains disabled; see
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-026 focused evidence: `cargo test -p ferric-browser-storage
--locked --offline` passed 267 tests on the current Unix source. The
shared-object, active-capture/GC, pinned-quota, interrupted-deletion,
history-clear, bounded-admission, descriptor-relative transition, same-inode
rewrite, injected sync-error, and cleanup-retry cases all ran in that suite.
This is focused package evidence only. Later continuation edits are not
covered by the earlier workspace gate, and per the current run instruction the
final gate remains deferred until all feasible implementation work is
addressed. VB-026 stays in progress pending final workspace verification and
ledger reconciliation. Non-Unix operation remains explicitly unsupported;
this is not evidence of same-UID race exclusion, kernel-generated fsync
failure, power loss, or a physical-volume/WAL hard cap. Broader filesystem
fault qualification remains a VB-091 concern. See
`docs/testing/versioned-browsing/VB-026.md`.

2026-10-06 VB-056 reduced-chrome status continuation: the command rejection
path now emits fixed generic copy instead of interpolating the expanded
command name into shared status. Added a source-boundary regression for the
fixed message and absence of formatting/name interpolation. The change and
test are authored but unbuilt/unrun under the final-verification deferral;
VB-056 remains in progress. See `docs/testing/versioned-browsing/VB-056.md`.

2026-10-06 VB-056 status privacy continuation: successful configured-action
status no longer exposes the configured target label or operation ID, and
spawn-start status no longer exposes generated operation IDs. Structured
action results retain the explicit fields; dispatch behavior is unchanged.
Source-boundary regressions are authored but unbuilt/unrun under the final
verification deferral. See `docs/testing/versioned-browsing/VB-056.md`.

The same 2026-10-06 VB-056 pass changed switcher success status to fixed copy
instead of including the configured action label. Dispatch/routing is
unchanged; its source-boundary regression is authored but unbuilt/unrun.
Selection-action completion likewise uses fixed status without its configured
target label or operation ID; worker completion behavior is unchanged. Its
source-boundary assertion is part of the same unbuilt/unrun test.

2026-10-06 corrected final campaign: after the first rerun found that a
source-boundary assertion still expected the now-removed pending-action
`target_name` field, the regression was updated to require the label not be
retained while explicit action results keep their typed target and operation
fields. `cargo xtask check --locked --offline` then passed on this corrected
tree: QML 208 passed / 8 capability-gated skips; reachable native archive,
capture, restore, cookie/HTTPS boundary and lifecycle probes passed; workspace
unit and doc tests passed. The 37-input release artifact manifest and
12-dependency license policy passed. The extension-activation viewless-download
journey remains skipped on tested Qt 6.10.3/6.11.2, and SameSite/Partitioned
cookie identity remains unqualified. The native mutation journey passed after
its fixture was changed to produce an observed DOM `textContent` mutation.
The bounded-journal/deletion/GC slice VB-026 now meets its supported-Unix
acceptance and is verified; VB-024, VB-045 and the other active slices
remain open for their remaining criteria. `validate_pack.py --next` still
reports no newly dependency/capability-ready task; continue reviewing active
work and concrete blockers without claiming the blocked extensions complete.

2026-10-06 VB-056 acceptance reconciliation: the same integrated gate built
and ran its declared cleanup/restart/corruption/symlink/late-callback,
post-commit sync-boundary, private-profile exclusion, exact diagnostics-preview
authorization, profile-retirement-during-hash, quota, and status/IPC/logging
sentinel regressions. QML passed 208 with eight capability-gated skips, the
reachable native lifecycle/archive/capture matrix passed, and workspace tests
and doc tests passed. The listed VB-056 acceptance is now verified for the
tested Unix/Qt configuration. Exhaustive diagnostics-sink and broader
cross-platform/power-loss fault qualification are not claimed; archive and
cookie capability gates remain disabled/unqualified as recorded elsewhere.

2026-10-06 VB-044 feasibility follow-up: inspected the exact Qt WebEngine 6.11.2
source revision used by the spike. Qt explicitly disables Chromium Background
Fetch and `ProfileQt::GetBackgroundFetchDelegate()` returns null, so toggling a
web feature flag cannot replace the crashing extension stimulus with a
supported page-less download source. This is pinned-source evidence only; no
runtime journey or capability promotion is claimed. The standalone extension
activation failure on Qt 6.10.3/6.11.2 and the missing null-page-to-durable-
ledger journey remain the concrete VB-044 blocker. See
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 isolated newer-runtime VB-044 recheck: rebuilt the disposable native
extension probe against signed Arch extra-testing Qt 6.12.0 / Qt WebEngine
6.140.0rc-1 packages extracted only under `/tmp`. The probe entered the public
`QWebEngineExtensionManager::setExtensionEnabled()` call and immediately
SIGSEGV'd in the isolated `libQt6WebEngineCore.so.6`; GDB did not resolve a
named Chromium frame. No popup, worker download, null-page callback or durable
ledger row was reached. This adds a negative result on a newer non-final
WebEngine RC, not evidence about final Qt 6.12 or a proven root cause. Keep
VB-044 in progress and archive capture disabled. See
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 VB-044 public API alternative check: a disposable native loopback
probe called public `QWebEnginePage::download()` through a `QWebEngineProfile`
on both installed Qt 6.11.2 and isolated Qt 6.12.0/WebEngine 6.140.0rc-1. Each
download completed with exact fixture bytes, but `QWebEngineDownloadRequest::page()`
was non-null in both native callbacks. This is not the required null-page
stimulus, and the Qt Quick profile used by Ferric has no public accessor for
the corresponding core profile/page needed to route this call through its
exact owner. Do not share profile storage or use private internals as a
workaround. The VB-044 gate is unchanged; details are in
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 VB-024 popup-owner publication hardening: popup registration now
uses a bounded fail-closed helper at the final publication boundary, not an
unguarded registry `filter()` after allocation. It reads each existing host
once, rejects malformed/over-limit state, deduplicates the exact host, and
catches registry access/write exceptions so the caller hides/destroys the
unopened popup rather than escaping the cleanup branch. Added focused QML
cases and updated the Rust source-boundary assertion. They are authored but
unbuilt/unrun under the end-of-goal verification deferral; no native popup
lifecycle qualification is claimed. See
`docs/testing/versioned-browsing/VB-024.md`.

The same VB-024 continuation hardened popup admission, owner-retention
inspection, registration, and unregistration to read each entry's host once
per pass. Registration and unregistration use bounded fail-closed helpers;
unregistration leaves the current registry intact if malformed state or
replacement assignment fails. Focused QML cases cover exact-owner add/remove,
throwing and stateful getters, setter failure, and refusal after process
shutdown begins/is approved. The final registration check repeats the
shutdown gate after popup allocation so the admission-to-registration window
cannot introduce a new owner during teardown. These cases remain
unbuilt/unrun. This keeps destruction-time cleanup fail-closed as well as
publication.

2026-10-06 VB-024 Save Page fallback scan correction: source review found that
the profile-wide quarantine scan treated a null root popup registry as empty,
which could misclassify unknown popup ownership as clean. It now requires a
bounded array on the process root, tolerates only an absent per-child registry
(popup ownership is process-wide), snapshots each popup host once, and returns
unknown for malformed/throwing registry, entry, or profile getters. Unknown
refuses the unpermitted-save profile fallback. Added focused QML and Rust
source-boundary cases; they are authored but unbuilt/unrun under the end gate.
See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 shutdown accounting continuation: process shutdown now uses
one bounded popup-host snapshot helper. Unknown/malformed/oversized or
duplicate-host state counts as an outstanding participant; shutdown refuses
to start if the initial snapshot is unknown. A valid snapshot closes popup
admission before entering the UI shutdown gate, and cancellation only traverses
popup owners from a valid snapshot. Added focused QML and Rust source-boundary
regressions; they remain unbuilt/unrun under the end-of-goal verification
deferral. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 restore-owner registration continuation: restore-window
capacity checks and child retention now fail closed on thrown/malformed owner
callbacks. Failed retention destroys the child. The production owner connects
release/discovery handlers before publishing the child and rolls back partial
signal wiring/publication on failure, avoiding a phantom owned child that can
hold shutdown open. Focused QML and Rust source-boundary regressions are
authored but remain unbuilt/unrun under the end-of-goal verification deferral.
This does not qualify native navigation or teardown races; see
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 secondary-download routing continuation: the restore-window
owner lookup now uses the bounded process-wide popup snapshot and returns
unknown for unavailable/malformed registry state or throwing owner access,
rather than treating the registry as empty and falling back to the secondary
profile UI. The request path therefore refuses/cancels ambiguous ownership
without reading Qt's view getter. A Rust source-boundary regression was added;
it is authored but unbuilt/unrun under the end-of-goal verification deferral.
See `docs/testing/versioned-browsing/VB-024.md`.

The bounded owner-tree `hostForDownload` scan also now distinguishes an absent
per-child registry from the process-root popup registry. It snapshots exact
host/view entries once and returns unknown when the required root registry is
unavailable or malformed, rather than losing a popup's retained request owner.
QML and source-boundary regressions are authored but remain unbuilt/unrun under
the end-of-goal verification deferral. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-055 media-scope disclosure: the versioning warning and the
accessible description for optional media-position restore now explicitly
label live, DRM-protected, adaptive-streaming, and device-backed media as
unsupported. The collector exclusions did not change. A focused panel QML
assertion is authored but unbuilt/unrun under the final verification
deferral; the unkeyed fingerprint privacy decision and VB-057 gate remain
open. See `docs/testing/versioned-browsing/VB-055.md`.

2026-10-06 VB-093 stale-certificate API hardening: removed the capability
requirement evaluator that could return support without an active runtime
fingerprint. The remaining evaluator requires the exact current fingerprint,
so old positive and negative probe facts cannot be consumed without an
explicit runtime comparison. Focused capability tests were adapted but are
authored/unrun under the verification deferral. Cross-runtime qualification
and VB-090 integration remain open; see
`docs/testing/versioned-browsing/VB-093.md`.

2026-10-06 VB-093 unknown-fingerprint rejection: runtime fingerprint
construction now refuses bounded placeholder tokens such as `unknown`,
`unavailable`, and `missing`, case-insensitively. This prevents a synthetic
unknown tuple from matching itself as if it were installed-runtime evidence.
A focused regression is authored but unbuilt/unrun under the end-of-goal
verification deferral; transition-runtime and VB-090 blockers remain.

2026-10-06 VB-093 full-tuple regression: added focused coverage that changes
each runtime fingerprint component independently (Qt, Qt WebEngine, Chromium
base, security patch, package revision) and requires stale support to resolve
as unqualified. The regression is authored/unrun under the end-of-goal
verification deferral; no cross-runtime qualification is claimed.

2026-10-06 VB-024 duplicate popup-owner rejection: bounded popup registry
admission, registration/unregistration, and approved-window retention now
reject duplicate host identities, matching the existing snapshot helpers'
fail-closed behavior. Focused QML assertions cover all four routes and are
authored/unrun under the verification deferral. Native popup lifecycle and
the broader VB-024 blockers remain open; see
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-045 consent disclosure: the capture warning and archive action's
accessible description now say saved archives may omit resources and are not
guaranteed to be atomic snapshots. This reflects the documented incomplete
resource/mutation coverage without changing capture behavior or its release
gate. Focused QML assertions are authored/unrun under the verification
deferral; see `docs/testing/versioned-browsing/VB-045.md`.

2026-10-06 VB-045 native consent copy: extended the Rust-generated one-use
capture disclosure for all eight scope/opt-in variants to warn that archives
may omit resources and are not guaranteed to be atomic snapshots. Added Rust
coverage across every disclosure variant and retained the panel/accessibility
copy assertions. All new tests are authored/unrun; archive capture remains
release-disabled. See `docs/testing/versioned-browsing/VB-045.md`.

2026-10-06 VB-044 public API audit: official Qt 6.11.2 references confirm
`QWebEngineProfile::downloadRequested` is a notification/decision signal, not
a request-construction API; the documented programmatic trigger is
`QWebEnginePage::download()`, whose callbacks were non-null-page in both
tested runtimes. Qt documents a null page only for downloads not triggered by
page content. This narrows the remaining stimulus to a proven non-page
engine/content route or a separately approved adapter; it does not resolve
the extension activation crash or change the release gate. See
`docs/testing/versioned-browsing/VB-044.md`.

2026-10-06 VB-024 poison-scan snapshot reuse: the profile-wide Save Page
quarantine scan now validates the exact popup registry value it read through
the bounded host/view snapshot helper, rejects duplicate popup identities
across its owner tree, and enforces a total 64-entry bound. This avoids a
second getter read and keeps malformed ownership unknown. Focused QML and
source-boundary regressions are authored/unrun; viewless callback and
lifecycle qualification remain open. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 profile download popup snapshot: all four profile-level
download resolution paths now obtain a bounded process-root popup snapshot and
refuse unknown registry state instead of treating it as empty. The router
independently snapshots browser host/profile/view/active-view and popup
host/view/profile fields, rejects duplicate host identities within/across
registries, requires an exact profile, and returns unknown on throwing ownership
getters. Notification focus
and live download metrics now use the shared snapshot too; its bounded record
includes opener UI. Added QML regressions for single-read routing, duplicate
records, throwing ownership access and opener-UI capture, plus Rust
source-boundary assertions for all consumers. These are authored/unrun under
the end-of-goal verification deferral; native viewless download and lifecycle
qualification remain open. The restore-owner child-registry branch also passes
the exact property value it already inspected into the snapshot helper, avoiding
a second read. The full owner-tree lookup now catches other throwing request or
descendant ownership getters and returns unknown; repeated popup hosts across
different scanned registries are ambiguous too. Focused QML/source-boundary
cases are still unrun. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-055 key-store boundary recheck: current `keyring` binary-secret
and HMAC APIs do not by themselves solve Ferric's key custody path. Capture
and restore compare media sources inside ApplicationWorld, where passing a
profile key would put it in renderer memory; moving raw media URLs to Rust
instead requires a new exact-document callback and must avoid blocking the GUI
thread on Secret Service. The current Secret Service draft allows unlocked
items to be accessible to another client, so this can only narrow detached-
sidecar guessing, not same-user/running-process access. No dependency, key, or
release change was made; a reviewed async/native-boundary design and explicit
threat model remain prerequisites. Details and primary references are in
`docs/testing/versioned-browsing/VB-055.md`.

2026-10-06 VB-055 media-source digest disclosure: visible and accessible
versioning copy, all eight fixed native capture-consent variants, and the
live-open confirmation now disclose that supported media matching stores a
deterministic unkeyed source-URL digest in sidecar metadata. The copy states
that it is not encryption, common URLs may be guessed, and matching sources
can be linked across sidecars. The final confirmation now includes both the
media and editable-draft disclosures when both options are selected. Focused
QML and Rust assertions are authored/unrun under the end-of-goal verification
deferral. No key lifecycle, media behavior, or release gate changed; the
security decision and VB-057 qualification remain open.

2026-10-06 VB-024 request-index fail-closed routing: a candidate download owner
must now expose object-valued active, pending and completion indexes; those
maps and its native page-save record are read once per candidate. Malformed/
missing maps, throwing identity entries, inherited rather than own request
entries, and an absent/non-array source tab-view registry return unknown
ownership rather than becoming an empty-state non-match. Added QML and Rust
source-boundary regressions; they are authored/unrun under the end-of-goal
verification deferral. Native viewless callback and teardown-lifecycle
qualification remain open. See `docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 restore-owner index parity: the recursive restore-window
download lookup now uses the same fail-closed active/pending/completion index
checks as the profile router, exact own-property request identity, and a
required array-valued child tab-view registry. Browser children require valid
Save Page tombstone indexes; popups may omit those capture-only properties,
but partially malformed tombstone state is unknown. Added QML cases for
malformed descendant and popup maps, inherited entries, and missing view
registries plus source-boundary assertions. Authored/unrun under the
verification deferral; native late-owner and viewless-request qualification
remain open.

2026-10-06 VB-024 live metrics ownership: live download lookup now requires
well-formed active/pending/completion indexes and exact own-property IDs with
object-valued requests across the bounded browser/popup snapshot. It matches
only owners bound to the active profile, so another profile's same numeric ID
cannot affect metrics. It rejects conflicting request objects, duplicate
owners, malformed registries and getter failures as unknown; the primary
host's one expected occurrence in its own browser registry is handled as the
same owner. Focused QML and source-boundary regressions are authored/unrun
under the verification deferral. This does not qualify native viewless
callbacks or late owner teardown.

2026-10-06 VB-024 secondary download owner: the restore-window resolver now
uses a shared exact-request helper over its validated active/pending/completion
indexes and bounded popup snapshot. Popup owners are profile-filtered; malformed
maps, repeated owners and same-ID/different-request collisions refuse. A clean
no-match still reaches the existing exact-profile fallback for a new request,
but completion requires a retained owner. QML helper and Rust source-boundary
regressions are authored/unrun. This does not qualify native viewless callbacks
or teardown timing.

2026-10-06 VB-024 completion-ledger admission: the shared shutdown-retention
helper now requires a bounded decimal request ID and valid object-valued owner
ledger, refuses to overwrite a different retained request under the same
profile-local ID, checks the exact identity after insertion, and catches
ledger/cancellation exceptions while refusing safely. Primary, secondary and
popup request handlers use it before download UI; popup no longer does an
unchecked direct completion-ledger write or opens UI after shutdown approval.
Shutdown-state getter failure also cancels while retaining the exact request.
Repeated notification for an already-retained exact request is suppressed
without cancelling the original request or opening a second chooser. Added
focused QML cases for collisions, duplicate notifications, malformed/throwing
ledgers, shutdown state, failed writes and cancellation fallback. All three
production wrappers return the resolver result, and a rejected/throwing owner
cancel falls back to the public request cancel method without double-cancelling
reported success. Rust source-boundary assertions enforce those paths.
Authored/unrun under the
end-of-goal verification deferral; this does not qualify native viewless
callback delivery or abnormal owner teardown. See
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 VB-024 shutdown-ledger inspection: the shared predicate now treats
missing/malformed pending, active and completion ledgers, invalid own request
entries and throwing entry getters as outstanding, so uncertain callback
ownership cannot authorize window release. Primary, secondary and popup close
checks use this predicate. Added focused QML cases and Rust source-boundary
assertions; they remain unbuilt/unrun under the end-of-goal verification
deferral. Native callback delivery/close timing remain open. See
`docs/testing/versioned-browsing/VB-024.md`.

2026-10-06 final verification continuation: the current-tree
`cargo xtask check --locked --offline` passed with 223 QML passes, zero
failures, eight gated skips, workspace Rust/doc tests, and reachable native
probes. `cargo xtask test versioning-profile-download` passed with exact
same-profile source/child Save Page routing when Qt exposed a null view plus a
separate streamed/cancelled child-view attachment. The Save Page route does
not qualify arbitrary content-originated viewless callbacks or their durable
ledger. Own-descriptor snapshots and source-boundary assertions passed. See
`docs/testing/versioned-browsing/final-campaign-2026-10-06.md`. The implementation
goal remains active; ordinary viewless-download evidence, WebRTC absence,
archive capability gates, product/security digest acceptance, and final pack
validation/reconciliation remain open.

2026-10-06 RTC evidence reconciliation: the latest source-pinned native RTC
fixture is already recorded as three fresh off-the-record runs in each default
and strict-site-isolation mode, and the exact Qt Quick owner fixture as three
fresh runs. `source-lock.json`'s nine retained log digests match the files in
the disposable target. No repeat engine build is needed for this fixture tuple.
The runs exercise bounded synthetic devices, exact-view lease/lifecycle and
renderer-loss cases only; complete WebRTC absence and production Rust ownership
remain unqualified, so VB-033 stays blocked. See
`docs/testing/versioned-browsing/VB-033.md` and
`docs/testing/versioned-browsing/engine-capability-spike.md`.

2026-10-06 RTC ApplicationWorld continuation: the three peer-creation legs in
the exact Qt Quick fixture now create and close their data-only peer connections
inside `WebEngineScript.ApplicationWorld`. Three fresh off-the-record processes
passed; the native peer registry observed the peer, and dispatch invalidated
the held lease before script execution. The updated fixture digest and run
roots are recorded in `tools/versioning-engine-spike/source-lock.json`, with
the scope and limits in the RTC reports. This narrows one script-world gap but
does not prove complete WebRTC/media absence or production ownership. VB-033
remains blocked and production live retention stays disabled.

2026-10-07 RTC ApplicationWorld media continuation: the exact Qt Quick
synthetic fake-microphone route now also runs in `ApplicationWorld`. Three
fresh off-the-record runs passed pending-permission refusal, active-stream
refusal, and fresh empty admission after track stop. The source-lock entry
records the updated fixture digest and disposable runtime roots. Combined with
the prior ApplicationWorld peer runs, this qualifies those tested browser-owned
script routes only; complete activity absence and production ownership remain
unproven. VB-033 remains blocked and live retention stays disabled.

2026-10-07 RTC cross-origin child continuation: the exact Qt Quick owner now
uses `view.mainFrame.children` and `WebEngineFrame.runJavaScript` to create a
data-only peer directly in the cross-origin child frame's `ApplicationWorld`.
Three fresh `probe-rtc-quick-oop` runs under strict `--site-per-process`
observed the peer in the exact-view native registry with at least two frames
and renderer processes, then reloaded the same view to a fresh empty registry;
one default run also passed. This is the implementation route to carry into
the production frame-ownership design, supported by Qt's QML WebEngineFrame
API. It does not prove exhaustive frame/media absence or production Rust
coordination, so VB-033 remains blocked and live retention stays disabled.
The new fixture hashes and disposable roots are recorded in
`tools/versioning-engine-spike/source-lock.json` and summarized in the RTC
capability reports.

2026-10-07 capture-state source follow-up: the pinned Chromium source has
`WebContentsObserver::OnFrameIsCapturingMediaStreamChanged(RenderFrameHost*,
bool)`, `WebContents::ForEachRenderFrameHost` (including BFCache/prerender
frames), and the internal per-frame
`RenderFrameHostImpl::HasMediaStreams(kCapturingMediaStream)` query. This gives
the patched Qt adapter a plausible start/stop invalidation plus complete-frame
reconciliation route for capture, beyond its current tested Qt media counters.
The follow-up test-only bridge compiled, but its dedicated strict OOPIF probe
did not qualify it: the synthetic RFH counter injection returned false after
the child page's empty lease, and the per-frame query remained clear; a second
run failed earlier at cross-origin empty-lease acquisition. The earlier actual
child-frame `getUserMedia` attempt returned `AbortError: Invalid state`. The
normal strict OOPIF peer/lifecycle control still passed after isolation. This
does not provide a native capture witness or production absence certificate;
the query is private, callback/lifecycle gaps remain, and data-only
PeerConnections are separate. Keep activity unknown and live retention
disabled. Exact source, patch, runtime and failure-log hashes are in
`tools/versioning-engine-spike/source-lock.json`.

2026-10-07 RTC child-media follow-up: five strict-site-isolation actual
fake-camera requests from the cross-origin child failed with
`AbortError: Invalid state`; no QML permission callback or live track was
observed. The child was complete, visible and secure, its effective camera
Permissions Policy was true in the latest run, both primary-page RFHs were
active, and Chromium `GenerateStreams` entry was logged. A no-strict-isolation
control reproduced the error. Qt source can map an empty authorized-device
set to this error, but the exact executed branch is unproven; the public
profile permission query cannot pre-grant nonpersistent video capture. No
working child-media/capture witness resulted. A strict-site-isolation
same-origin child control failed identically, so cross-origin policy and OOPIF
separation alone do not explain the symptom. Keep VB-033 blocked and live
retention disabled. See the detailed evidence and hashes in VB-033 and
`source-lock.json`.

2026-10-07 controlled follow-up: three strict-site-isolation fake-camera runs
did observe a live cross-origin child track, an active exact-view capture
query, refusal from the inverse no-capture query, then a stopped/ended track,
cleared capture state and a newly admitted empty lease. The QML marker records
`childCaptureTrack=true childPermissionCallback=false`. The test-only Qt grant
mode emitted no permission trace; an exact-origin deny-mode control remained
`AbortError: Invalid state` and likewise emitted no trace. The capture and
cleanup route is now witnessed in the disposable fixture, but the permission
authority and exhaustive absence guarantee remain unknown. This does not
unblock VB-033 or change the production capability gate. Full hashes and
commands: `tools/versioning-engine-spike/source-lock.json`.

Correction, 2026-10-07: the direct stderr trace files subsequently confirmed
that the forced test grant and deny controls executed their exact-origin Qt
branches. The normal path was also traced through `ASK`, the Qt Quick
`permissionRequested` signal, and the QML handler. The handler's comparisons
were wrong: Qt 6.11.2 exposes `WebEnginePermission.PermissionType` as a scoped
enum, so `WebEnginePermission.MediaVideoCapture` was undefined. The production
controller now uses the scoped enum path, with a QML mapping regression test.
That test passes, and three fresh ordinary strict-site-isolation runs—with
both test-only override flags absent—now grant through the QML callback and
observe fake child-camera start, exact-view capture refusal, track-stop cleanup
and fresh empty admission. This supersedes the preceding permission-unresolved
statement. The result proves this tested callback/capture route only; the
private Chromium observer is not yet a production-qualified exhaustive
absence source, and lifecycle/callback-loss coverage plus production Rust
ownership and release qualification remain. VB-033 remains blocked and the
live-retention capability remains disabled. See
`tools/versioning-engine-spike/source-lock.json` and
`docs/testing/versioned-browsing/VB-033.md` for the retained digests.

2026-10-07 cookie T0 follow-up: the isolated Qt 6.11.2 fixture now verifies
one acknowledged source write before `GetAllCookies` and the existing forced
native write after its complete-list callback but before child seeding. Three
fresh disposable runs passed eight checks each; exact child read-back stays at
the three-record cut while a separate source copy sees four. This is a serial
ordering witness only—not cross-interface writer exclusion, profile-generation
ownership, child writer isolation, crash-durable publication, or production
seed approval. The source lock and VB-075 retain the runtime, probe and run
hashes. Cookie production gates remain disabled.

2026-10-07 public-writer-during-copy-orchestration follow-up: the disposable
fixture starts the multi-step public copy-existing probe and a public
`QWebEngineCookieStore::setCookie` back-to-back before pumping the Qt event
loop. The copy operation performs preflight reads before the actual complete
list request; three fresh eleven-check runs included the synthetic writer in
the exact five-record cut and later exact source copy. The fixture permits a
complete four- or five-record cut, but does not directly race the setter
against the actual `GetAllCookies` request. See VB-075 and
`native_t0_copy_operation_writer_followup` in the source lock.

The same 2026-10-07 source audit traced Qt's private complete-list bridge into
Chromium CookieMonster: the full snapshot is copied from ordinary and
partitioned maps on its thread-checked owning sequence before callback, while
set/delete operations use that sequence. This supports a linearizable T0 for
the tested pinned internal route, but it is not a public Qt contract or a
complete writer inventory and does not solve Ferric owner-generation binding,
durable publication, or shipping approval for a maintained engine fork. See
the audited source hashes in the new VB-075 source-lock entry; production cookie
gates remain false.

2026-10-07 VB-055 key-lifecycle implementation progress: added a Linux-only
profile-scoped Secret Service key primitive in the storage crate, using the
existing persistent UUID and exclusive profile lock, explicit Secret Service
backend selection, read-back provisioning, zeroized buffers, redacted Debug,
bounded URL canonicalization, and domain-separated HMAC-SHA-256. Its unit tests
are authored but deliberately unrun until final verification; no credential
was touched. The capture/restore path now passes source URLs transiently to
native Rust for HMAC conversion before sidecar admission, drops media when the
key is unavailable, and uses a load-only restore lookup plus final exact-source
recheck; the ApplicationWorld never receives key bytes. Helper version 5 and
the visible/accessibility consent disclosures now describe the keyed equality
hint and residual threat boundary. No tests have run yet. Rotation/revocation
fault coverage, locked/missing-store and deletion-retry probes, final
verification and VB-057 qualification remain. Persisted hints now carry an
explicit `hmac-sha256-v1-kN:` format/key-epoch label; legacy unkeyed and
future-format hints remain parseable but cannot authorize a media seek under
the active format. Key epochs advance on explicit rotation or when capture
discovers that the active credential was removed. Rotation/deletion also
revokes in-process cached key instances, and the Qt adapter refuses stale keys.
The keyring-returned secret allocation is immediately wrapped in zeroizing
storage. A durable marker is written before key provisioning, allowing profile
deletion to skip Secret Service entirely for profiles that never used media
capture; failed credential cleanup retains both marker and deletion manifest.
The root `reset-data` operation now removes all marked keys before clearing
storage and fails closed if the credential store is unavailable.
See `docs/testing/versioned-browsing/VB-055.md`.

2026-10-07 VB-055 reset-order hardening: `reset_owned_contents` now validates
all resolved root directories before deleting any marked Secret Service key,
so an already-unsafe root cannot cause credential deletion followed by an
immediate reset refusal. The existing unsafe-root regression now includes a
malformed key marker to prove that root validation wins before marker/key
cleanup. The marker scanner also has an authored malformed-identity refusal
case, and corrupt marker cleanup is asserted to preserve its retry marker.
The marked-key scan now shares the process-wide key-operation guard with
capture/rotation so a same-process provisioning operation cannot race the scan
and have its marker erased by reset. The VB-055 QML matrix also adds explicit
non-finite-duration and negative-current-time rejection assertions, replaces
the actual media DOM node during observation, and checks for no raw URL leakage.
It also adds zero-call counters for `getUserMedia` and `getDisplayMedia` beside
the playback counters. These authored tests remain intentionally unrun ahead
of the final gate.

2026-10-07 VB-055 lifecycle fault seam: the key primitive now uses a private
credential-store trait while all production entry points remain fixed to the
Linux Keyring/Secret Service implementation. Test-only in-memory storage
zeroizes copied key values and injects one-shot get/set/delete failures.
Authored cases cover marker-before-provision, unavailable-store no-fallback,
rotation failure/success and cached-key revocation, missing-key restore versus
explicit-capture epoch recovery, deletion retry, and reset marker retention.
The storage-root reset test asserts key-cleanup failure leaves owned files
untouched. No credential access occurred; the tests are recorded as authored at
this checkpoint.

2026-10-07 VB-055 focused lifecycle verification: `cargo test -p
ferric-browser-storage --locked --offline` passed all 286 tests and doc tests,
including the injected credential-store lifecycle cases, reset cleanup
retention, and profile deletion recovery. The first full run exposed that
marker-free `Temporary` roots were being treated as key-store failures during
ordinary profile deletion. Cleanup now accepts the no-marker no-op for
temporary roots but still refuses an unexpected marker. Transactional profile
deletion, post-commit startup recovery, and both temporary-root marker cases
pass. `cargo fmt --all -- --check` also passes. The current offscreen
`tst_versioned_page_state.qml` run passed 31 checks with 7 expected
loopback-fixture skips, including replacement, invalid-time, source-redaction,
and zero playback/camera/microphone/screen-capture calls. No real credential
was read or written. The core page-sidecar suite also passed 19/19 and the
application restore-planner suite passed 18/18. A Qt adapter `cargo check`
found and prompted fixes for an immutable request borrow across mutable
normalization and then a duplicate `Clone` derive; after those fixes, the
adapter check passed. The two focused native normalization tests passed 2/2;
they inject the fingerprint function and do not invoke the credential worker
or system keyring. Include the adapter in the final workspace gate. This
focused evidence does not qualify arbitrary media, the live Secret Service
backend, VB-057, or production/release gates; keep those closed and the
workspace/release gate deferred until the remaining feasible implementation
and blockers are reconciled. See
`docs/testing/versioned-browsing/VB-055.md`.

2026-10-07 VB-055 Secret Service prompt-safety rework and isolated probe:
review of the exact pinned `keyring` 3.6.3 source showed its lookup helper
unlocks locked matches, potentially waiting on a user prompt. The production
adapter now uses `dbus-secret-service` 4.1.0 directly, with an encrypted DH
session and zero prompt timeout; it requires an unlocked default collection,
rejects locked/ambiguous items, and never invokes unlock. Lookup attributes
preserve the prior `target`/`service`/`username` identity. The dedicated
`tools/test-isolated-media-fingerprint-keyring.sh` harness creates a fresh
D-Bus session and temporary XDG tree, then runs two bounded ignored integration
tests. Both passed: one qualified provision/read-back, rotation and revocation,
cross-profile distinction, deletion, and fail-closed load/deletion when the
default collection is locked; the locked collection and retry marker remained
intact. The other overrode the test process with a nonexistent private D-Bus
socket and confirmed provisioning fails without leaving a marker. The focused
media-fingerprint suite passed 18/18 with these two isolated tests intentionally
ignored. Only a random test credential in the disposable service was touched;
no desktop credential was accessed. This qualifies the local GNOME backend and
missing-bus endpoint, not active-bus/no-service-owner behavior, other Secret
Service providers, item-only locking variants, arbitrary media, or release
gates. Production capture/recovery remains disabled. See VB-055.md for source
references and harness details.

2026-10-07 VB-055 integrated media/key-store journey: the filtered native Qt
journey now passes with the direct Secret Service adapter in the isolated
GNOME Keyring harness. It provisions the profile key asynchronously before
dispatching the page collector, commits the keyed local media sidecar, reopens
the profile and restores the same paused media position without playback. The
journey exposed a `view` local accidentally retained by the refactored QML
helper; the retry now uses the view stored in the capture and checks it is
still the exact current tab view before starting collection. The test command
`FERRIC_TEST_NATIVE_MEDIA_JOURNEY=yes
tools/test-isolated-media-fingerprint-keyring.sh` passed the native restore
journey and the two isolated Secret Service tests then configured in the
harness. After adding the active-bus/no-owner case, a later standalone
`tools/test-isolated-media-fingerprint-keyring.sh` invocation passed all three
cases: unlocked/locked GNOME Keyring behavior, a missing bus, and a live private
bus with no Secret Service owner. The latter uses a no-activation bus config so
it cannot start the desktop keyring. No desktop/user credential was accessed.
Formatting, shell syntax and `git diff --check` passed. VB-055 is now verified
for its scoped implementation acceptance. Arbitrary site media, other
providers, item-only lock variants, the remaining source-hint privacy review,
VB-057 and final workspace/release gates remain open; production
capture/recovery remains disabled. See
`docs/testing/versioned-browsing/VB-055.md`.

2026-10-07 VB-044 extension witness containment: review of the official Qt
WebEngine development index found only 6.140.0 beta/RC artifacts, not a final
release; the previously tested RC crashes immediately after extension
activation. `xtask` now requires the explicit
`FERRIC_TEST_NATIVE_VIEWLESS_EXTENSION=yes` opt-in before launching that
crash-prone native probe, so routine checks on Qt 6.12+ cannot enter the same
unqualified activation path by default. This is a test-safety improvement,
not a capability result. No new native download journey was run. Formatting,
pack/JSON integrity, and whitespace checks passed. The backlog selector still
reports no dependency/capability-ready task, so VB-044 stays blocked pending a
qualified null-page/profile-owner and durable-ledger journey, or equivalent
evidence from an approved public adapter. Production archive capture remains
disabled. See `docs/testing/versioned-browsing/VB-044.md`.

2026-10-07 scope and branch continuation: the pack now has 99 slices. VB-024
is verified only for additive new-window structural restore; current-window
replacement moved to blocked VB-096 because it still needs qualified live
retention. VB-044 is verified only for the exact permitted MHTML Save Page
route; ordinary page-origin download-to-ledger moved to blocked VB-097 after
the public MV3 extension activation fault. Hidden-parent clean branching moved
to VB-098. VB-058 is verified for its bounded typed branch-profile lease and
off-record Quick profile owner seam, including focused Rust, QML and native
profile/cookie-store tests; this does not enable `branch`. VB-059 is in
progress as a visible-parent, parent-active transaction: core lineage now
accepts a child with its actual separate native application/profile owner,
and its hidden preparatory QML window refuses mixed startup modes, eager
navigation (including the URL binding) and publication before native binding
and lineage admission. The application now embeds the typed profile lease and
checks a current normal parent against a distinct bound ephemeral child and
first tab; five focused lease tests pass. The durable fork, Qt adapter
cross-owner handoff, authorized child navigation and lineage/coverage
publication remain to implement. VB-061 and VB-064 were detached from the
hidden-retention chain for their visible-parent variants; VB-068 has an
explicit VB-023 prerequisite for its restore module. The pack validator
passes. `versioning.clean_branch` and all other production capability gates
remain disabled. See VB-058.md and VB-059.md.

2026-10-07 VB-059 transaction continuation: the application now queues an
exact single-tab structural fork, waits for durable publication, and checks
the source tab, logical page and native profile generation again before
requesting a child. A typed Qt handoff validates a distinct hidden child
`BrowserUi`, its bound ephemeral owner and first tab, then preflights and
publishes child lineage plus parent root/branch state. Close and abort paths
release the Rust lease without replacing the live parent. QML creates only a
hidden clean child with a separate memory-only profile; a bounded timer waits
for native binding and the exact QML profile/view lease before lineage
publication, safe-GET navigation admission, registration and show. A child
navigation enqueue failure destroys it while leaving the source active.
Default builds still refuse `branch NAME --clean`; the route is test-only
behind `unqualified-clean-branch-test`. Default and feature-on application
command-route tests passed, as did the prior application transaction tests;
the Qt bridge passed `cargo check` before the latest navigation-result guard,
and QML parsing passed with existing cross-component warnings. Rebuild,
focused tests, and an integrated native command-to-child-to-close journey
with failure teardown remain. Do not mark VB-059 verified or enable the
production clean-branch gate based on these partial checks. The durable fork
commit is retained as a normal user commit if child creation later fails.
See `docs/testing/versioned-browsing/VB-059.md`.

2026-10-07 VB-059 native placeholder continuation: the test-feature browser
build passed. A disposable offscreen normal profile accepted `branch
native-trial --clean` and, after the durable fork, its `query windows` live
registry showed the unchanged original normal window and `tabid-7` plus a
distinct one-tab ephemeral child with a different owner token. This is one
real child-publication observation, not a safe-GET or close certificate.
`window-close` only requested the normal confirmation dialog; its IPC
acknowledgment did not remove the child, and a subsequent `tab-close`
acknowledgment did not establish teardown. An isolated headless Weston
attempt registered accessibility but Ferric queries/branch commands failed,
so it cannot close the evidence gap. The compositor and all three disposable
profiles created for these attempts were stopped and deleted. The application
branch suite passes 8/8 with wrong-owner close and abort checks, and the
default and test-feature command routes each pass. A source `about:`/`file:`
URL is now a blank child placeholder rather than an attempted safe GET; the
native child navigation guard admits only canonical HTTP(S) safe GETs and
returns its enqueue result so a rejected hidden child is destroyed. The
visible success status explicitly says `parent-active` and structural-only
child with no copied local site data. VB-059 remains in progress and its
production gate remains closed until native safe-GET, confirmation/close and
failure teardown journeys pass. See VB-059.md.

2026-10-07 VB-059 counted native safe-GET continuation: `xtask`'s loopback
`/versioning/document` fixture now has a bounded exact-document GET counter
with a focused unit regression. A disposable offscreen normal Ferric process
navigated original `tabid-7` to that page and reported it loaded. The counter
was 1 before `branch native-counted --clean`, 2 after, and stayed 2. The
registry showed a further distinct one-tab ephemeral child while the
original normal window/tab identity remained unchanged. Read-only durable
manifest inspection showed the fork descriptor was replayable. This is
native evidence of safe-GET child navigation, not page-state cloning or
cookie/storage inheritance. Both fixture servers and the disposable
browser were stopped and its profile removed. The prior instruction that a
native safe-GET journey remained open is superseded for this narrow case;
confirmed close/teardown and native POST/unsafe-URL, allocation-failure and
navigation-rejection cases remain open. Keep VB-059 in progress and the
production clean-branch gate disabled. See VB-059.md.

2026-10-07 VB-059 confirmed native close continuation: a disposable normal
profile under the current Wayland compositor opened a clean child. The
ordinary `window-close` command surfaced the child confirmation dialog;
a PID-scoped AT-SPI action pressed only its visible confirmation button.
The live registry returned to the unchanged normal parent and original
`tabid-7`. Reusing the exact same branch name then opened a distinct new
ephemeral child, giving native evidence that the prior branch record and
name reservation were released. The second child and finally the parent
closed through their own confirmation dialogs; the Ferric process exited 0.
The process and disposable profile were removed. This supersedes the prior
close-journey gap for the ordinary one-child path, not failure/nested-child
teardown. A focused application regression models same-URL POST proof
invalidation and a token-bearing unsafe URL; both durable forks yield only
unsafe-navigation placeholders while preserving the original source. Native
POST/unsafe-URL, allocation-failure and rejected-navigation teardown
journeys remain. VB-059 stays in progress and the production clean-branch
gate stays disabled. The branch application suite now passes 9/9. See
VB-059.md.

2026-10-07 VB-059 native unsafe-URL continuation: a disposable normal
profile loaded the loopback document with `?token=synthetic`; the exact
document GET count was 1. `branch unsafe-token --clean` published a distinct
ephemeral child, but the count stayed 1. Read-only durable manifest
inspection showed `placeholder / unsafe-navigation`; the parent window and
tab remained unchanged. Child and parent closed through their ordinary
confirmations with process exit 0, and the fixture/profile were removed.
The fixture also has an accepted-form-POST counter and focused unit test,
but the native form submit did not occur through the available WebEngine
accessibility action, so no native POST result is claimed. The application
proof-invalidation test is the current POST evidence. VB-059 remains in
progress for native POST, child-allocation failure and rejected-navigation
teardown; the production gate remains disabled. See VB-059.md.

2026-10-07 VB-059 QML failure-path continuation: the production hidden-child
handoff now calls `CleanBranchPreparation.js`, shared with a focused QML
fixture. Its 7/7 checks cover allocation throw, wrong child owner identity,
owner-publication refusal and abort, rejected navigation destroying the
hidden child before registration, failed publication, and placeholder
success without navigation. This tests the same QML decision code, not a
native allocation or navigation fault. A Qt adapter rebuild and
post-refactor native smoke remain at this checkpoint; keep VB-059 in
progress and its production gate disabled. See VB-059.md.

2026-10-07 VB-059 post-refactor smoke: the default Qt adapter check and the
test-feature browser build passed with the shared QML helper. A fresh
disposable offscreen normal profile accepted `branch refactor-smoke --clean`
and exposed the unchanged original normal window/tab plus a distinct
one-tab ephemeral child in the live registry. The process and disposable
profile were removed. Keep VB-059 in progress for the remaining native
fault/POST qualification and keep the production gate disabled.

2026-10-07 VB-059 native POST no-replay continuation: the initial form
accessibility click was inconclusive, so the bounded loopback fixture gained
`/versioning/form-auto-post`, a single delayed synthetic form submission.
A fresh disposable offscreen Ferric process loaded `/versioning/form/result`
in original `tabid-7` with complete loading; the fixture accepted-POST
counter was 1. `branch native-post --clean` published a distinct ephemeral
child while the POST counter stayed 1. The durable fork descriptor was
`placeholder / unsafe-navigation`, and the original window/tab identity
was unchanged. The browser, fixture server and test profile were removed.
This supersedes the earlier native POST gap for the scoped single-tab path.
Native fault-injected allocation/navigation rejection and final release
qualification remain; the production gate stays disabled. See VB-059.md.

2026-10-07 VB-059 scoped acceptance: the visible-parent single-tab
implementation is verified behind the explicit test build feature. Its
specified POST/unsafe-source cases have application and bounded native
evidence; allocation and navigation-rejection decisions run through the
same production QML helper exercised by the 7/7 failure-path fixture;
native positive safe-GET, placeholder, confirmed close/reuse and separate
off-record profile/cookie-store journeys pass. The parent stays live and
unchanged, and the child is structurally forked with no copied local site
data. The production `versioning.clean_branch` gate remains disabled;
native fault injection, broader scopes and release qualification are not
claimed. The backlog now records VB-059 verified and unblocks VB-060,
VB-061, VB-063 and VB-066 as not-started; VB-062 remains blocked by VB-050.
Continue with VB-060, the lowest-numbered newly eligible slice.

2026-10-07 VB-060/VB-063 implementation continuation: VB-060 now has a
published cross-owner line registry in `BrowserApplication`, name/UUID
resolution scoped to the exact source window/profile, case-folded names,
native child-owner/first-view validation, and a typed test-feature `switch`
route for parent CLI/IPC. The root and child command bars resolve through
their exact source application; a shared `CleanBranchSwitch.js` activation
helper admits only a matching live QML parent registry entry or child lease,
with no fallback and no navigation/profile reassignment. The application
branch suite passes 9/9, including colliding local TabIds, wrong child,
source mapping and stale post-close target; the QML switch fixture passes
7/7. A disposable Wayland run accepted parent-targeted child and main
switch commands, rejected an unknown name and retained the same normal and
ephemeral window/profile identities. Background IPC did not establish the
compositor's actual focus outcome, so VB-060 is `in-progress`, not verified.
The process and its disposable profile were removed.

VB-063 now preflights clean-branch popup allocation against the exact
published off-record profile, first view, child permission UI and request
interceptor. A popup starts hidden and is opened only after the same exact
check; nested popups stay in that branch. Sensitive popup callbacks recheck
ownership and deny/cancel stale permission, certificate, authentication,
WebAuthn, desktop-media and file requests. The production decision helper's
QML fixture passes 6/6. A disposable native child loaded the loopback popup
page, but its AT-SPI button action did not yield a trusted compositor user
gesture or popup frame, so the native popup attempt is inconclusive. The
browser, fixture server and exact disposable profile were stopped/removed.
VB-063 is `in-progress`; the production clean-branch gate remains disabled.
The feature-enabled browser build passed after the popup helper extraction,
and a final rebuild after the last sensitive-callback guard was started.
Read `docs/testing/versioned-browsing/VB-060.md` and `VB-063.md` for bounded
evidence and remaining native journeys. The backlog/pack validator passes
with 51 verified, 39 blocked, 7 not-started and 2 in-progress tasks.

Next implementation priority is VB-061, an independent eligible slice. The
current `FerricBrowserWindow.qml` clean child has one static WebEngineView,
so window-scope grouping needs an exact multi-tab child-host design (one
off-record profile, ordered views, selected page, per-page load status) before
the command can be enabled. Do not fake it as unrelated child windows or
publish runtime tabs without native views. VB-060 compositor focus and
VB-063 native popup/redirect/prompt qualification can be completed at the
final verification stage; neither is a license to enable production gates.

2026-10-07 VB-061 window-plan start: `BrowserApplication` now has a bounded
`CleanBranchWindowPlan` derived from an already prepared window-scope
structural manifest. It checks the exact current native owner/profile UUID,
source tab order and liveness, logical-page correspondence, selected page,
safe-GET/placeholder descriptors and the 128-page bound. Its focused 128-tab
regression passes and refuses reversed order, wrong profile and a closed tab.
This is metadata only; it allocates no child and does not enable the
window-scope command. The native one-profile/multi-view child host and
per-page load outcomes remain; VB-061 is `in-progress`. See VB-061.md. This
entry supersedes the preceding sentence calling VB-061 `not-started`.

2026-10-07 final check for this continuation: VB-061's plan now validates
the manifest itself, requires the prepared application revision, and refuses
forged replayable URLs without an exact live source-tab safe-GET proof. Its
128-page regression plus the other branch application tests pass 10/10.
The default-build branch/switch gate test, core switch-effect debug-redaction
test, QML switch fixture 7/7, QML popup fixture 6/6, `xtask` 35/35, Qt
adapter `cargo check`, feature-enabled browser build, `cargo fmt --check`,
`git diff --check` and pack validator all pass. The last feature build was
started before the final VB-061 proof check; the final Qt adapter check and
application tests completed after it. No production capability was enabled.
The next implementation work remains VB-061's one-profile/multi-view native
child host, then per-page load/failure accounting. VB-060 and VB-063 retain
their documented native qualification gaps.

2026-10-07 VB-061 continuation: the application now queues one durable
window-scope branch capture from the validated bounded plan, with one reserved
child-profile lease. The durability callback rechecks exact source owner,
ordered tabs, selected tab, target generations, logical pages and safe-GET
proofs before emitting `WindowDurable`; changed source state cancels the
lease. `publish_clean_branch_window_lineage` can publish an ordered set of
child pages into one ephemeral profile and branch record only when the child
runtime window has exactly the expected live tabs, order and selection. The
single-tab publisher rejects group forks. The Qt event path fails closed on
an unexpected group event; there is still no window-scope branch command or
native multi-view child host. Application branch tests pass 12/12; Qt adapter
`cargo check --locked --offline` passed after the event-path edit. See updated VB-061.md. The next
work is the native one-profile/multi-view host, lazy page navigation and
per-page outcome accounting; do not mark VB-061 verified yet.

2026-10-07 group-owner preparation: the branch-popup source check now keeps
the exact published first-view lease as its anchor but also admits a listed
additional page view only with the same live off-record profile, host, child
UI and request interceptor. The child host's extra-view list is initially
empty; no group view allocator or window-scope command was added by this
change. The expanded QML ownership fixture passes 8/8, including forged and
stale extra-view refusal. VB-061 and VB-063 both remain `in-progress`; see
their evidence files. The Qt adapter `cargo check --locked --offline`
passed after the QML edit.

2026-10-07 plan execution continuation: VB-066 now has a one-use, bounded
structural export from an exact linked off-record child to its normal parent
owner. The preview makes no durable write and shows only destination profile
and page count; confirmation rechecks parent/child native profile bindings,
lineage, window/tab ordering, target generations and safe-GET proofs before a
new normal-profile commit. The child `commit` route fails closed instead of
falling through to ordinary private persistence. Application branch tests
passed 15/15, including cancellation, wrong destination, unrelated private
owner, closed child owner and two-tab durable reopening after child teardown;
the feature-enabled Qt adapter check and browser build passed. VB-066 is
`verified` for this test-feature structural scope, with production gates still
off; see `docs/testing/versioned-browsing/VB-066.md`. The selector currently
has no ready not-started slice because VB-060/061/063 are in progress and the
remaining tasks depend on those or blocked strict capabilities.

A separate disposable native focus attempt published a distinct child and
accepted `switch`, but the compositor remained focused on ChatGPT even after
an explicit exact-address focus request. It does not prove VB-060's required
parent/child focus journey. The test process and exact disposable profile were
removed. VB-060 stays in progress. The shipping-backend decision checkpoint
in ADR-0016 keeps the public Qt path, rejects automatic fork adoption and
lists the exact owner/race/fault/security/maintenance work needed before a
patched backend could be reviewed. The locally retained research engine tree
is about 13 GiB; that is a development footprint, not a release-size claim.

2026-10-07 staged-plan verification checkpoint: the native page-capture
collector and application installer now share the exact `include_media`
choice. An unavailable profile key cannot authorize media observation or
install a media category through a no-media challenge; pending key loading
still defers the collector. The focused Qt callback suite passed 16 active
tests, the focused application capture suite passed 39/39, and the QML JSON
contract allowlist was updated for the bounded clean-branch export preview.
The complete `cargo xtask check --locked --offline` then passed: 253 QML
passes (8 expected skips), native profile/clean-branch/archive/cookie/HTTPS
probes, release-input and license checks, workspace Rust tests and doc tests.
The native MV3 ordinary viewless probe remains skipped because of the
documented activation crash. See
`docs/testing/versioned-browsing/final-campaign-2026-10-07.md` and updated
VB-055. Pack validator still reports 99 tasks, 52 scoped verified, 3 in
progress, 39 blocked, 5 not started and no enabled production capabilities;
`--next` is null. VB-060 focus, VB-061 multi-view child host, VB-063 native
popup/permission journeys and strict backend gates remain outstanding.

2026-10-07 VB-061 host continuation: the test-feature `branch NAME --clean
--scope=window` route now uses one hidden tab-capable off-record child with
ordered views, selected tab and a single exact parent/child lineage handoff.
The first view anchors the lease; additional views must match the child
profile/UI/interceptor/native creation host before publication. Proved safe
GETs are admitted only after publication; placeholders remain blank. Group
popup and sensitive-prompt routes recheck the exact branch owner, and failed
hidden bootstrap removes its strong window-owner reference. The feature Qt
check/browser build, application 15/15, focused popup 9/9 and owner 30/30
passed. A disposable offscreen native three-tab test published two ephemeral
three-tab children, kept the normal parent unchanged and observed one counted
safe-GET replay; after termination/restart, only the normal window returned
and no branch-named WebEngine storage path existed. This does not qualify
compositor focus, ordinary confirmed close, native popup/permission flows,
failed page loads or background lazy navigation. VB-060/061/063 remain in
progress; production gate remains off. See updated VB-061/VB-063 and the
post-check note in `final-campaign-2026-10-07.md`. Rerun the broad gate after
this continuation before any final release claim.

2026-10-07 VB-061 deferred-navigation follow-up: the complete post-change
`cargo xtask check --locked --offline` passed (256 QML passes, zero failures,
eight expected skips, native probes, workspace Rust and doc tests), and a
fresh feature-enabled browser build passed. A disposable offscreen source
with one counted GET page and two blank tabs produced a three-tab clean
child. Its GET counter stayed at 1 on child publication, rose to 2 only when
the child selected the background page, and stayed 2 on repeat selection.
An attempted first-view-anchor tab close left the child at three tabs.
Restart exposed only the normal window and no branch-named on-disk WebEngine
profile. The exact disposable profile was permanently removed. VB-061 remains
in progress pending native failed-page/cleanup, popup/prompt, confirmed-close
and compositor-visible focus journeys. VB-060/VB-063 and strict engine gates
also remain open; no production capability was enabled. See VB-061 and the
final-campaign checkpoint for precise scope.

2026-10-07 VB-061 original-page outcome accounting: the child now holds a
bounded, URL-free pending/loaded/failed status per original stable tab ID,
after checking exact view/UI/profile/tab binding. Reordered/closed tabs
cannot assign a later load to another original page. The focused QML
preparation fixture passed 9/9; the second full `cargo xtask check --locked
--offline` passed on this source with 257 QML passes, zero failures, eight
expected skips, native probes, workspace Rust and doc tests. A separate
ordinary `window-close` CLI request displayed confirmation and left the child
registered; the offscreen CLI did not exercise Proceed. Native failed-URL
reporting, confirmed close, popup/prompt, compositor focus and strict engine
capabilities remain open. The final feature-enabled browser rebuild passed.

The final feature-enabled binary was also smoke-tested with the new
original-page outcome hook in a disposable offscreen process. A controlled
typed source GET advanced the local fixture counter to 8; publication of a
one-page clean child advanced it to 9, with both normal and ephemeral
windows registered and no QML diagnostics. The direct startup URL had made
earlier repeat requests, so only the controlled delta is evidence. This
does not inspect the private outcome array or qualify a failed child URL.
The exact fixture/browser processes were stopped and the disposable profile
was permanently deleted.

A second disposable negative journey completed one counted source GET,
stopped that exact loopback fixture, then branched. The normal parent and
ephemeral child remained registered without QML diagnostics, but the public
CLI cannot inspect the child's private per-page outcome. Treat this as
containment only, not failed-page-reporting qualification. The exact browser
process and disposable profile were stopped and permanently removed.

2026-10-07 VB-061 focused native failure/close qualification: a one-use
loopback fixture now lives in a test-feature Qt journey. Its source GET
succeeded and the child replay connection dropped; a content-free internal
diagnostic observed the original child page's `failed` outcome, with the
normal parent still live. This test exposed a real attribution bug: Qt
completed an internal blank/error document after replay failure and the
initial recorder overwrote the failure as `loaded`. The recorder now gates
on the planned safe-GET start and stable tab ID, classifies a non-HTTP
terminal document as failed, and keeps the first terminal result. The
focused QML fixture passed 9/9 and the native test passed. The same native
test requested a real child window close, observed confirmation, proceeded,
and saw child teardown with the parent intact. Its disposable profile was
removed. VB-061 remains in progress for mixed group failure, popup,
permission, reorder and broader owner-fault journeys; production remains
off. Rerun the complete workspace gate after this change.

The first complete post-change gate found an unrelated archive-test harness
race: it broke on a “capture saved” status before a queued captured-tab close
callback. The exact journey passed on rerun. The test now waits for its
already-required one-tab state inside the existing 45-second deadline,
without relaxing the production requirement. The corrected focused archive
journey and the subsequent complete `cargo xtask check --locked --offline`
passed (257 QML passes, zero failures, eight expected skips, native probes,
workspace Rust and doc tests). The final feature-enabled browser build also
passed. Strict public-Qt capability gates and VB-060/061/063 status are
unchanged.

2026-10-07 VB-063 native popup continuation: the feature-gated Qt journey
opened a local scripted popup through a Qt-delivered click in a published
clean child. The top-level popup's profile, permission UI and request
interceptor matched the exact off-record child owner; the normal parent
remained live. Child close entered its shutdown gate and requested popup
close. The popup's page-state probe explicitly required “Close anyway”; the
test made that second decision. This exposed that an approved hidden popup
QML object remained registered indefinitely, so the child could not finish
shutdown. `FerricPopupWindow` now defers object destruction until outstanding
download and shutdown request owners clear, then unregisters on destruction.
The focused native test passed with popup and child both retired, parent
still live. `CleanBranchPopupOwnership.sourceReady` now refuses a host after
shutdown approval. Focused QML fixtures passed 9/9 branch-popup and 30/30
window-owner checks. The test-only bridge exposes no page content. A separate
experimental handoff in the older `FerricBrowserWindow` path was removed
because it was not qualified and could close its UI before popup callbacks.
VB-063 remains in progress for target=_blank, redirect, saved-trust and
permission-prompt native journeys; VB-061 remains in progress for mixed
group, reorder, failed allocation and broader lifecycle journeys. No
production capability is enabled. Rerun the complete workspace gate and
feature-enabled browser build on this final source before claiming them.

2026-10-08 VB-061/VB-063 continuation: the expanded feature-gated native
Qt journey now passes twice. For VB-061 it creates a three-tab parent and
one off-record child, with a selected blank placeholder and two proved
background GETs. Selecting the one-use page produces a child-only failed
replay, then moving that tab changes the child's core tab order without
changing its original stable-ID `failed` outcome. Selecting the other safe
page records `loaded`; the blank original stays `pending`. The three-tab
parent remains intact through confirmed child close. The test-only driver
must process `tab-open` via ordinary runtime model synchronization before
consuming its navigation effect; an earlier immediate-consumption attempt
was rejected by the native assertions. For VB-063, scripted `window.open`
and clicked `target=_blank` each follow a local 302 to a final page while
the popup retains the exact child's off-record profile, permission UI and
interceptor. A normal-profile exact-origin durable notifications allow is
not inherited: a fresh child-owned notification prompt appears, no parent
prompt appears, and explicit deny/confirmed close completes. The test-only
bridge exposes owner IDs and prompt state, not page content. The prior
complete workspace gate passed after the production popup-release fix; rerun
it after this expanded native test and rebuild the feature-enabled browser.
VB-061 remains in progress for failed native allocation, first-view anchor
replacement and broader privacy/lifecycle/focus qualification. VB-063
remains in progress for certificate/saved-trust and cross-origin OAuth-style
redirect qualification. No production capability is enabled.

The final complete `cargo xtask check --locked --offline` passed on the
expanded source, including QML, native probes and archive journeys, release
inputs, workspace Rust tests and doc tests. The feature-enabled `ferric-browser`
build also passed. `cargo fmt --all`, `git diff --check`, pack validation and
all pack SHA-256 inputs passed. The focused feature-gated native branch journey
passed twice on the same source. These checks do not qualify the public-Qt
strict capability gaps or promote VB-060/061/063 to verified.

2026-10-08 native driver follow-up: the same feature-gated branch journey
passed once through Cargo and five more times in fresh processes after
removing startup/view-commit races in its loopback fixture. It waits for the
normal source's committed cookie-bearing page, takes a distinct typed safe-GET
proof and counts only uncookied requests deliberately refused for the clean
child. The test explicitly confirms a separate page-state “Close anyway”
prompt if the three-tab child raises one after window-close confirmation.
After all children retire, removing the tab-capable factory makes a later
clean window branch refuse without a child or parent damage. This covers an
absent factory, not late partial allocation cleanup. The complete post-change
`cargo xtask check --locked --offline` passed (257 QML, zero failures, eight
expected skips, native probes/journeys, release/licence, workspace Rust and
doc tests); the feature-enabled browser build passed. VB-061/VB-063 remain in
progress and production stays off.

2026-10-08 cross-origin/anchor continuation: the feature-gated native journey
passed through Cargo and five fresh-process repeats after adding two absolute
redirects, `127.0.0.1` → `localhost` → `127.0.0.1`, for both scripted and
`target=_blank` popups. The fixture counts intermediate and callback GETs;
the popup's exact off-record child profile, permission UI and interceptor stay
bound throughout. This is a loopback OAuth-shaped round trip, not external
OAuth interoperability or saved-certificate trust. In the mixed group, a
user `tab-close` on the selected original first-view anchor was refused by
the core with no eligible unpinned tab, leaving all three child IDs and order
unchanged. It does not directly qualify the separate QML anchor guard or
late view replacement. VB-061/VB-063 stay in progress and production remains
disabled. The complete post-extension `cargo xtask check --locked --offline`
passed (257 QML, zero failures, eight expected skips, native probes/journeys,
release/licence, workspace Rust and doc tests), and the feature-enabled browser
build passed.

2026-10-08 late-allocation native continuation: the feature-gated journey
passed through Cargo and five fresh-process repeats after the startup driver
stopped requiring a completed `about:blank` load. Its separate typed normal
source GET remains the safe-replay proof. For VB-061, a hidden two-tab clean
child reached exact native binding without publication; removing its pending
extra-view component made it retire without lineage while the parent retained
two tabs. A retry from that parent published a two-tab ephemeral child and
closed after the presented confirmations. This is late component loss before
extra-view creation, not partial-created-view cleanup or first-view
replacement. For VB-063, each published child explicitly opens the
popup/permission fixture before the click, so exact child owner evidence is
not misreported as parent-to-child fixture replay. Production gates remain
off. The final `cargo xtask check --locked --offline` passed (257 QML, zero
failures, eight expected skips, native probes/journeys, workspace Rust and
doc tests), as did the feature-enabled browser build on this revision.

2026-10-08 first-view and nonterminal replay continuation: the native
three-tab journey directly called QML's `closeTabAtIndex(0)` after reordering
another tab. The first-view anchor guard refused and preserved all three
stable IDs, separately from the core `tab-close` refusal. An earlier repeat
exposed a child-only refused GET that never produced a terminal Qt callback;
the planned original page stayed `pending` on a provisional blank view. A
started-only 15-second timeout now records `failed` only after exact current
view/owner recheck; placeholders and completed pages do not expire. The QML
decision fixture passed 10/10, and the combined native journey passed through
Cargo and five fresh processes on that source. A final disposable fixture
then held the first uncookied child replay connection open beyond the
deadline; exact planned start and timeout-specific `failed` were observed
before the socket was released. This passed through Cargo and five further
fresh-process runs. The mixed-page route separately retains a dropped
connection failure. Partial-created-view/first-view replacement, compositor
focus and broader lifecycle faults remain open. The complete broad gate
passed after the production timeout code (258 QML, zero failures, eight
expected skips, native probes/journeys, workspace Rust and doc tests), as did
the feature-enabled browser build. The final post-fixture broad gate also
passed (258 QML, zero failures, eight expected skips, native probes/journeys,
workspace Rust and doc tests), as did the feature-enabled browser build.
No production capability is enabled.

2026-10-08 full-vision backend direction: the owner approved pursuing a
maintained, version-pinned Qt/Chromium backend. This is **not** an approval to
ship the current diagnostic overlays or promote any strict capability. The
new `tools/versioning-engine-backend/verify-and-run.sh` checks exact observed
Linux x86_64 research library/process and QML/resource/locale hashes, refuses
a mixed install, and can launch Ferric only in a disposable offscreen safe-mode
environment. Its integrity check passed; a bounded Ferric smoke process
survived 18 seconds, and live process maps confirmed pinned Core, Quick and
QtWebEngine QML plugin paths. Its focused exact-view host fixture passed 18/18
on that tuple, with three Chromium rejected-widget-message diagnostics. A
disposable public-Qt control also passed 18/18 and emitted five messages of
the same class, so they are not patch-specific but remain open for release
review. This is development handoff/host-compatibility
evidence, **not** RTC absence, cookie snapshot/seed/merge, viewless download
or production release qualification. ADR-0016 and
`docs/testing/versioned-browsing/engine-capability-spike.md` record the
decision and evidence. Extract a minimal non-diagnostic production patch,
connect Ferric exact owner/generation handles, then qualify capabilities
separately while keeping public Qt the default fail-closed backend. In a
disposable smoke process the actual pinned renderer reported `NoNewPrivs: 1`,
`Seccomp: 2` and one seccomp filter; an intermediate zygote reported
`Seccomp: 0`. This narrow observation does not complete sandbox review.
Four backend slices VB-099–102 were added to the proposal, standalone slice
appendix and backlog with unchanged existing dependencies/capability gates.
The 103-task pack validator passes and `--next` selects VB-099. This repairs
the work queue for the authorized backend route; it does not claim any of
VB-099–102 is implemented or that the 39 previously blocked tasks are done.
The narrow `check-no-probe-api.sh` release-hygiene check correctly refuses the
current research install because exported `ForProbe` symbols remain; VB-099's
preparatory evidence is in `docs/testing/versioned-browsing/VB-099.md`.
