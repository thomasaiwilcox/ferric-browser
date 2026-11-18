# Site policy baseline — 8 November 2026

P00 status: Complete after independent exit review on 11 November 2026.
Completion covers baseline evidence, contracts and test preparation. Original
performance, availability, lifecycle and platform failures remain recorded;
they do not become qualification passes. P01 is the next active phase.

## Execution-order audit — 10 November 2026

P00-01–12 and P00-GATE are complete after independent evidence and exit reviews.
P01 remains unqualified. Independent P02 results and partial P03 work are
preserved, but further later-phase feature implementation is held pending P01
feasibility/review. The owner plan's Section 13 records the execution-order
error, recovery order and 11 November P00 exit approval.

The read-only evidence audit found that later records labelled P00 include
modified production source. They cannot establish an unchanged historical
baseline. Keep their results and source attribution; do not turn later fixes
into original baseline passes.

| Required evidence | Audit result / next eligible action |
| --- | --- |
| Initial build, core/config/IPC, format, dependency policy, native CMake | Original pinned-source records retained; no repeat needed solely for convenience. Native result remains offscreen only. |
| Workspace all-target Clippy | Original-source recovery pass: `p00-pinned-clippy.json` / `.log`, exit 0 in 1,153.356s; all 2,074 source files unchanged. Existing Qt/GCC native warnings remain in the log. |
| `xtask check` and workspace tests | Original-source recovery pass: `p00-pinned-check.json` / `.log`, exit 0 in 1,525.298s; 1,304 Rust tests passed, two opt-in ignores, 1,231 QML tests passed, all three native profile/download CTests passed, all 2,074 source files unchanged. The earlier changed-source aggregate failure remains a separate historical result. |
| `xtask test engine` | Original-source recovery pass: `p00-pinned-engine.json` / `.log`, exit 0 in 430.789s; 570 Rust tests passed with two opt-in ignores, 1,231 QML tests passed, native router CTest passed in 19.67s. All 2,074 source files unchanged. |
| `xtask test application` | Original-source aggregate finished with 96/102 groups passed and six failed. Source and CLI artifact checks matched. Both IPC probes skipped by CTest failure passed separately. Detailed failure boundaries are below. |
| `application-wayland`, `wayland-input`, `wayland` | The original nested application command failed its active-window setup after a successful build. The input command found the compositor's virtual keyboard protocol unavailable. The standalone startup command passed in 447.130s including its debug build. |
| `xtask package arch`, explicit `--temp-basedir` launch | Original package metadata validation passed in 2.778s. The temporary launch passed in 3.835s and observed browser-driven profile removal. See `p00-pinned-package.json` and `p00-pinned-temp-launch-owned-rpc.json`. |
| P00-11 performance | Original component, command, cold Hint, combined refresh, activation, idle, lifecycle, ready-input, corrected rooted render and provider evidence are recorded at their prescribed DPR values. Availability failures and positive partials remain explicit, without invented percentiles. Both provider summaries, final source/artifact postcheck and full P00 exit review are complete. |

### Current performance receipt index — 11 November 2026

Paths in this table are relative to `target/site-policy-baseline`.
`dpr{1,2}` denotes the two separate DPR files. Each completed summary retains
its precise measurement boundary, raw-sample statistics and source receipts;
the detailed records below explain failures and environment limits. This is
an index of baseline evidence, not a performance qualification pass.

| Workload | Baseline and threshold records | Outcome / remaining work |
| --- | --- | --- |
| QML collector, fresh, action and overlay components | `p00-pinned-qml-perf-dpr{1,2}-summary.json`; `p00-pinned-component-budgets.json` | Both original suites recorded. Component boundaries exclude the complete browser journey; profiled records are excluded. |
| Rust decode, allocation, setup, ranking and snapshot | `p00-pinned-rust-perf-debug-summary.json`; `p00-pinned-rust-perf-release-summary.json`; `p00-pinned-component-budgets.json` | Three sizes, 10 warm-ups and 100 samples in each build profile. |
| Normal command key to Quick frame | `p00-v7-plain-baselines-1/command-render-baselines.json` | Both DPR values, 1,000 samples each. Existing p95/p99 ceilings are 20/50 ms; paired no-policy p95 ceilings are 12.243/18.810 ms. |
| Cold Hint key to usable overlay | `p00-v7-plain-baselines-1/cold-hint-dpr{1,2}-baselines.json` | All 500-target rows exceed the existing 200 ms p95 ceiling. The 1,000-target rows completed; 5,000-target first warm-ups failed. |
| 1,000-target passive refresh after 1,000 mutations | `p00-v7-plain-baselines-1/passive-refresh-availability.json` | Both DPR values fail the first warm-up; no valid samples or percentile ceiling. |
| 500-visible passive refresh after 1,000 mutations | `p00-v8-combined-refresh-1/verified-baselines.json` | Both DPR values completed 100 samples. Paired no-policy p95 ceilings: 817.632/974.330 ms. |
| Ready prefix, selection, text query and queued prefix | `p00-v10-ready-rooted-baselines-1/ready-dpr{1,2}/verified-baselines.json` | Both DPR values independently reviewed: 32 full rows and eight unavailable 5,000-target rows. Each completed row has its own frozen reference. |
| Selected Hint Enter to exact DOM click receipt | `p00-v9-full-lifetime-activation-1/activation-dpr{1,2}/verified-baselines.json` | Both DPR values independently reviewed. Includes fresh proof/action/receipt polling; 5,000-target preparation failed. No native trust qualification. |
| Rooted render: shadow, slot, same-origin, focused foreign, clipping | `p00-v11-rooted-baselines-1/roots-dpr{1,2}/` | Both DPR terminal summaries independently reviewed; final matrix postcheck matches. Failed rows retain their original outcomes. |
| Rooted provider collection and fresh proof | `p00-v11-rooted-baselines-1/root-stages-dpr{1,2}/` | Both DPR terminal summaries independently reviewed: each has 40 full, five partial and ten unavailable rows. Final matrix postcheck matches. |
| Final corrected-matrix source and artifacts | `p00-v11-final-postcheck.json` | Current 2,079-file source inventory, application binary, engine archive, browser and all four runtime bindings match the selected build. Matrix remains `failed-workloads`. |
| No-Hint blank-page idle | `p00-pinned-idle-before-lifetime-attempt-2/verified-report.json` | Two verified 120-second intervals. CPU/resource observations and the source timer lifecycle are separately attributed. |
| Ordinary/foreign Hint and shared/private window lifecycle | `p00-v9-full-lifetime-activation-1/lifetime-dpr1/verified-lifetime.json` | Ordinary/foreign 1,000-cycle and shared/private 100-cycle rows reviewed. Window/renderer/FD/PSS growth remains an adverse baseline finding for P15-07. |

Section 11 remains the frozen rule: no-policy p95 reference is baseline plus
`max(2 ms, 10%)`; matched collection/fresh p95 reference is baseline plus
`max(10 ms, 25%)`. Any stricter existing absolute ceiling wins. Apply references
only to the same boundary, fixture and environment. Unavailable or incomplete
rows have null reference ceilings. The 100-sample rows support p95 and max;
only the 1,000-sample command runs above claim p99. Native synchronous routing
and new policy/capture/draft workloads retain their later owning phases.
Independent read-only review confirmed the completed references, their scope
and frozen formulas. Rooted receipt paths identify the corrected matrix and
its terminal summaries.

The detached worktree `target/site-policy-baseline/p00-pinned-source` has HEAD
`f2c5d6deebd97eb861084503f25613a6a819949d`. Original functional checks used its
unchanged 2,074-file source inventory. The current measurement worktree retains
that production source plus the eight enumerated test paths in
`tools/performance/p00/p00-measurement-overlay-v11.json`; it is not a clean
checkout. Use its original lockfile and separate Cargo/CMake outputs. Preserve
the older `legacy-source` archive and all current feature work. Do not import
P02/P03 production changes into this baseline. Record exact source/artifact
hashes and Section 10.6 metadata for new runs. Genuine baseline failures remain
failures; this audit authorizes no threshold, timeout or workload reduction.
The three-attempt limit still applies to failing gates.

Recovery execution uses `p00-pinned-run.py` and separate
`target/site-policy-baseline/p00-pinned-target` Cargo outputs. The runner records
the complete 2,074-file source inventory, its own hash, selected SDK/compiler
and desktop fingerprints, command, exit status and source postcheck. The
record/log are `p00-pinned-clippy.json` and `p00-pinned-clippy.log`; this original
source Clippy run passed, with the v1 runner retained as
`p00-pinned-run-v1.py`. The subsequent original-source `xtask check` passed;
its record/log are `p00-pinned-check.json` / `.log`. No feature production source is copied into the
baseline checkout. Later runner records also hash produced executables/archives
before and after each command; source mismatch makes the record fail.

The following paragraphs retain historical recovery checkpoints; their pending
statuses describe those attempts. The current receipt index above and v11
reproduction instructions below identify the current evidence and inputs.

Read-only harness review identified a seven-file measurement-only overlay:
the current performance translation unit, runner and two HTML fixtures, plus
four slot declarations/one callback-result member, its callback assignment and
the performance source/optimization CMake hunk. Pinned initialization, routing
and helpers remain intact. The prepared patch
`p00-measurement-overlay.patch` passes `git apply --check` against the pin;
`p00-measurement-overlay.json` records its exact seven input/output hashes.
It has **not** been applied, built or exercised. Ready Hint input and other
missing workloads remain missing. Broker tracing must be a separately
identified diagnostic build with the original 1,000ms deadline and 1,000
mutation workload; it cannot supply unprofiled performance claims.

The separately prepared three-file broker diagnostic overlay is
`p00-broker-diagnostic-overlay.patch` (hashes and unchanged deadline recorded in
`p00-broker-diagnostic-overlay.json`). It passes applicability checks and
independent read-only review. Its
opt-in operation IDs distinguish attach initialization/lease checks, root/child
collection, dirty polling and discovery, with matching submit/callback/timeout
and late/retired completion records. It preserves guards and single completion
and logs only bounded browser-owned metadata. Callback latency includes
transport/event-loop delay, and logging perturbs timing. Remove this entire
overlay, including its QML comments, before unprofiled pinned measurements;
unsetting the trace environment variable alone does not restore pristine source.

The prepared v3 measurement patch was superseded before application or runtime
qualification: its 500-marker assertion after prefix filtering contradicted the
original default `Hide` behavior. An initial activation draft also incorrectly
expected a DOM click from ordinary Link navigation; it was never integrated or
run. The v4 candidate (`p00-measurement-overlay-v4.patch` / `.json`) corrects
those assumptions. Its 15 label-prefix, Tab-preview and genuinely queued-prefix
rows retain 500 established/unfiltered markers and validate the natural
matching cluster representatives after filtering. Five additional rows use
non-navigating buttons and exact DOM ID/name/count receipts for the production
selected-Hint → fresh validation → semantic-click journey. It retains failed
sample events/queue facts and checks unchanged navigation/selector subjects.
The runner records all source files before/after,
validates actual DPR 1/2, and rejects any non-harness source difference from the
pin for full timing, including diagnostic code with tracing unset. Only smoke
may admit the three explicitly recorded diagnostic paths. The unchanged-source
preflight accepts all 2,074 original files; patch applicability/Python syntax
and v3 source-guard review pass. V4 workload review approved the corrections.
Qualified Qt 6.11.2 C++ syntax-only checks passed for the v4 performance
translation unit/header (7.207s) and separate diagnostic broker unit/header
(2.602s); exact commands, hashes and logs are in `p00-overlay-syntax.json`.
Native Qt/GCC warnings remain in those logs. Neither patch has been applied to
the pinned checkout; full archive/harness builds, linking and runtime remain
pending. Text-query timing, compositor presentation and Hint-input
p99 remain outside these new rows and unqualified. Final activation now has a
draft measurement only. Source preflight
and artifact mtimes do not prove a binary's producer: retain the actual pinned
Cargo/CMake build logs, exact selected archive/headers and linked-binary hashes
as separate source-to-artifact evidence before any performance run.

The source guard was also checked against both actual worktrees without edits:
it accepts the 2,074-file original checkout and rejects the 2,236-file later
feature worktree (277 differing non-harness paths). Exact runner hash/results
are in `p00-production-source-guard-probe.json`. This is a source-control
check only; it does not qualify performance or an artifact's producer.

The original-source `xtask check` completed with exit 0 in 1,525.298s. It passed
15 qualification-tool tests, QML (1,231 passed, zero failed/skipped), two
profile-lifetime CTests, one download-source CTest and the complete workspace
tests (1,304 passed, zero failed, two opt-in ignores across 18 Rust groups,
including doc-test groups). The ignored tests are the explicitly invoked S17
Hint pipeline benchmark and S25 deterministic label-ergonomics model. All
2,074 source files remained unchanged. The source manifest and selected
executable hashes are in `p00-pinned-check.json`; this does not establish
application/platform/performance coverage or close P00. The required original
`engine` task also passed separately in 430.789s: 570 Rust tests, 1,231 QML tests
and the native-router CTest passed, with the same two opt-in Rust ignores.
Its 2,074-file source postcheck matched. The exact native executable hash/link
command is retained separately in `p00-pinned-engine-native-artifact.json`;
the runner's older artifact table used the wrong native executable basename,
so that table alone does not attest it.

The original-source `application` task has started as
`p00-pinned-application.json` / `.log`. Its legacy CLI probes default to
`ROOT/target/release`, while Cargo uses the separate pinned target directory.
An explicit ignored build-output alias now binds those paths to the same
pinned-produced release executable; no source or existing directory was
replaced. Exact setup is in `p00-pinned-build-path-alias.json`, and the runner
records both resolved paths and executable hashes. The v2 runner used for
the completed check/engine runs is retained as `p00-pinned-run-v2.py`.
The original application workflow finished in 10,600.077s with exit 1.
CTest passed 96 of 102 groups. All 2,074 source files matched at exit and the
release CLI alias and actual executable had matching hashes. The runner
record, full CTest log and failure list are indexed by
`p00-pinned-application-terminal-summary.json`. The pinned release engine,
browser and Qt harness built successfully. The full offscreen group
passed in 2,540.07s: 922 passed, zero failed, 40 explicitly skipped (including
setup/cleanup). The separate DPR2 group also passed in 7.12s. The completed
offscreen block and skip reasons are preserved in
`p00-pinned-application-offscreen-group.json` / `.log`; all 2,074 source files
still match the original manifest. These are group results, not a successful
application aggregate. Specialized-runner skips remain separate from passes.
The separate original `application_lifetime` group passed in 50.86s: its
1,000 F/Escape cycles ended with Qt descendants unchanged at 9,171, empty
renderer element/observer/window collections and scope/tracking false after
the original GC step. This is the existing early-visible/cancellation test;
it does not replace the v7 established-overlay and intermediate resource
controls. The original frame DPR2 group also passed separately in 52.20s.
The original `application_frame_lifetime` group passed in 284.09s, with all
ten 100-cycle progress checkpoints through 1,000 and zero failed/skipped tests.
Its source asserts zero native frame leases after every close and at the end.
It does not inspect child-realm observer/maps or intermediate memory retention;
those remain separate v7 measurements.

Two original-source groups have now hit their existing aggregate deadlines:
`application_page_windows` at 600.27s and `application_page_search` at 180.13s.
Independent source/log review found no assertion failure or Qt fatal in those
groups. Windows completed the first 101/135 data rows before interrupting
`window-mutations`; search completed 31 substantive cases before interrupting
`private-forward`. Neither cutoff establishes that the interrupted case hung.
The original plan for 34 Windows suffix rows
and five search selectors is preserved in
`p00-pinned-timeout-followups-before-group60.json`. Independent review of the
completed full offscreen group found all 39 selectors passed. Its environment
matches the failed search group, so repeating those five search cases is no
longer needed. The 34 Windows rows were attempted with the original inherited
Wayland environment and 600s bound. The driver requested focus, but Qt's
`probe.isActive()` prerequisite failed before any of those rows ran. The
follow-up exited 1 in 18.187s with matching source and artifact hashes
(`p00-pinned-windows-suffix.json` / `.log`). The 34 rows have offscreen coverage.
Their Wayland coverage remains open. Both original aggregate failures remain
in the record.

While that unchanged-source workflow runs, a separate scaled root fixture was
prepared at `p00-root-render-targets.html` (facts/hash in
`p00-root-render-draft.json`). It puts counted controls inside two real open
shadow hops, assigned slots, two same-origin frame hops, or two alternating
loopback-origin frame hops; its clipping case distinguishes N counted targets
from N+2 physical buttons. It uses correlated layout/focus readiness, stale
callback rejection, descendant pending propagation and frame-load fencing.
JavaScript syntax and independent source review pass. It is not integrated or
exercised in WebEngine, and proves neither broker authority nor 500 visible
markers. Adding it requires a separately reviewed eighth measurement-file
allowlist entry; the sealed v4 overlay and pinned checkout remain unchanged.

The prepared v5 overlay (`p00-measurement-overlay-v5.patch` / `.json`, eight
measurement files) integrates that fixture as 25 independent rooted-render
rows while preserving the v4 cold/ready/activation rows. It checks observed
connected shadow roots, actual slot assignments/depth, exact leaf identities,
two-hop frame paths/foreign native lease chains, viewport bounds and finite
delegate coordinates. It records truncation and marker counts and keeps the
existing 200ms ceiling for 500 visible markers. Entry is the public
`hint buttons` command; these rows do not claim native-key latency.
Applicability/Python/fixture-JavaScript syntax and independent source review
pass; the v5 C++ build and browser execution remain pending. Component
collection inspections/fresh timings remain separate required controls.

The source-reviewed v6 candidate (`p00-measurement-overlay-v6.patch` / `.json`)
adds those controls independently: 25 collection rows, 25 fresh-proof rows and
five explicitly separate one-leaf fresh availability controls. The one-leaf
subject requires `control=fresh` and cannot replace a prescribed-size workload.
Validated observations are retained before later operations; unsuccessful rows
have no completed full-run percentile. Exact retained identity, Core target,
selector/layout stability and acknowledged tracking teardown are checked.
The runner validates typed records and complete metric matrices, rejects nested
non-worktree source roots, and uses the complete pinned Git tree. Syntax/source
checks do not establish C++ build, browser behavior or performance; those remain
pending. None of these drafts is applied to the running baseline checkout.

The v6 runner's nine synthetic parser checks passed; exact inputs, runner hash
and individual retained metadata are in
`p00-parser-guard-probe/summary.json`. They cover a valid smoke record,
non-object JSON, non-finite/boolean/overflowing latency, invalid DPR,
missing/malformed root matrices and a nested source root. The archive check was
stubbed and fake executables printed crafted records: this is parser evidence
only. All eight v6 files have also been materialized and hash-checked outside
the original worktree (`p00-measurement-v6-materialization.json`), without a
compiler, linker or browser run.

Two dedicated original-source invocations additionally failed at driver setup:
`application_native_focus_freshness` (3.71s) and
`application_native_grouping_behaviour` (3.40s). Read-only source/log review
places both at the initial probe's second identity lookup returning
`reject-identity`, before focus dispatch. `initTestCase()` had passed; the
driver then deliberately sent SIGTERM. These stack dumps do not establish an
independent product crash (`coredumpctl info 3384578` found no core). All sixteen
selected substantive rows passed in the earlier Windows prefix. Keep the
dedicated invocations failed. A probe-destruction race is possible, but is not
proven; diagnose that boundary before any retry rather than broadly rerunning
successful rows or weakening identity admission.

The two terminal Hyprland grouping proofs failed after focus dispatch was
acknowledged. T101 reached singleton setup and could not observe its sentinel
as active. T102 reached singleton setup and failed to confirm one of its two
owned focus requests. The driver emits the same error for both T102 call sites.
Neither proof reached its grouping assertions, and the other fixtures do not
cover those compositor proofs. The review identified a bounded focus
diagnostic to distinguish call site, dispatch acknowledgement, mapped identity
and active identity before a repeat of either proof.

One T101 diagnostic ran with the original driver, SDK and 180s bound. It
reached sentinel focus at line 522 and received `requested`. None of the 71
original polls observed that stable identity as active. The same identity
still resolved and was mapped after failure. The run exited 1 in 4.878s with
all source files matching the pin (`p00-pinned-t101-focus-diagnostic.json` /
`.log`). The cause of the compositor's focus outcome is still unproven.

The original `application-wayland` command built the engine and harness, then
failed `initTestCase()` because the nested compositor did not activate the Qt
probe. No substantive cases ran. It exited 1 in 346.300s, including the build.
The separate `wayland-input` command exited 1 in 12.742s. Its nested startup
smoke passed and `wtype` reported the virtual keyboard protocol unavailable.
Both commands matched all 2,074 source files at exit. Their individual records
are `p00-pinned-application-wayland.json` and `p00-pinned-wayland-input.json`,
with their logs. The standalone `wayland` startup check passed in 447.130s,
including its debug build, with matching source files at exit. Its bounded
startup succeeded and editable-page input remained unavailable.

The prepared `p00-pinned-followups.py` retains separate results for the 34
remaining Wayland suffix selectors and the two IPC probes skipped after an
aggregate failure. The search mode remains available, but its follow-up is
unnecessary after the completed full offscreen coverage above. Read-only review
passes after requiring original terminal
source postchecks and exact final artifact hashes, restoring the original
selected environment (including absent overrides), and adding owned-group
termination/reaping plus interrupted terminal records. Original runner and
CTest environment snapshots match in
`p00-pinned-application-runtime-environment.json`; Windows inherits
`wayland;xcb`, while search uses its original offscreen CTest overrides.
Syntax and actual rejection of overlap with the live original workflow passed
(`p00-pinned-followup-guard-check.json`). The skipped IPC probes passed in
13.348s and 12.096s, respectively, with matching source and artifacts
(`p00-pinned-ipc.json` and `p00-pinned-window-ipc.json`, with their logs).

Lifecycle source review found that the original cancellation/final-only tests
cannot supply intermediate resource trends. The v7 measurement
candidate (`p00-measurement-overlay-v7.patch` / `.json`, still eight harness
paths) preserves the 107 timing rows and adds an explicit `--kind lifetime`:
1,000 established ordinary and 1,000 established focused-foreign Hint cycles,
plus 100 shared-profile and 100 private-profile window transitions. It records
warmed/intermediate/final renderer maps/observers, browser/provider state,
native profile ledgers, weak destruction observations and observed process
RSS/PSS/thread/FD counts. Exact Button identity, label and visible delegate
checks precede closure. Public native frame handles read the same document
after the unmodified production close; no manual tracking cleanup or GC is
inserted before observations. Hidden released hosts are reported separately
from destruction. This one-control lifecycle workload does not replace the
500-visible-marker workload, qualify named durable-profile switching, or prove
callback retirement or detached-DOM collection. Memory snapshots are observed
and non-atomic; unavailable subjects remain explicit.

The lifecycle snippets passed source review after correcting a secondary-window
API error, strengthening the rendered endpoint and retaining failed release
facts before assertions. Final combined v7 source/runner review passes after
also requiring unique typed window-cycle indices and the exact warmup matrix.
Applicability and Python syntax pass; 18 predecessor runner checks, eight
targeted corrected-matrix checks and five synthetic state-reader checks pass.
The predecessor patch/source are preserved with their original hashes. Records are
`p00-parser-guard-probe-v7/summary.json` and
`p00-parser-guard-probe-v7-matrix/summary.json`, plus
`p00-lifetime-state-reader-probe.json`. These use fake producers/ordinary
JavaScript objects, not WebEngine. The v7 measurement translation unit and
header subsequently passed an actual Qt 6.11.2/GCC 16 syntax-only check in
8.34s (`p00-measurement-v7-syntax.json` / `.log`), with all eight materialized
source hashes unchanged afterward. It ran at nice19 alongside the original
functional aggregate; no timing samples were taken. Existing native compiler
warnings remain unsuppressed. The older v4 syntax result remains separate
evidence. The first full v7 build is recorded below.

The package metadata command above ran alongside the original functional
aggregate. Cargo reused the existing optimized xtask (0.17s); no compiler build,
native test, install or performance measurement was started by that command.
The remaining standalone platform paths and explicit temporary-profile launch
still require their own original-source records.

The prepared `p00-pinned-temp-launch.py` performs that explicit debug
`cargo run -p ferric-browser -- --temp-basedir` path with offline/locked Cargo
and an isolated instance. It owns a short private TMPDIR/XDG tree, verifies the
temporary socket peer and executable, checks public start-page/live-diagnostic
readiness, then uses the original test-only Quit helper to confirm the rendered
production prompt. Browser-driven temporary-base removal is checked before
harness cleanup. The qualified SDK is explicit before XDG isolation, and the
owned helper/SDK plugin path is applied after the wrapper sets its environment.
Read-only source review, syntax, actual SDK/plugin environment selection and
rejection of overlap with the running application workflow pass
(`p00-pinned-temp-launch-preflight.json`). Synthetic process-group tests also
pass: stubborn descendants and descendants of an exited leader are retired,
while an unrelated process remains live
(`p00-pinned-temp-launch-retire-probe.json`). An earlier probe read raced a
disappearing `/proc` entry; its separate failed observation is retained. These
are harness checks. The first actual launch reached its owned socket, then
the separate CLI `--basedir` readiness query hit the durable layout check.
The second launch reached public RPC and received the expected default empty
tab projection for a transient profile. Both attempts and their logs remain
available. The final launch used the same Cargo command and set
`ipc.private_queries=true` temporarily through the verified owned socket.
The active tab then reported the ready built-in start page. The helper
confirmed rendered Quit and the browser removed the temporary profile before
harness cleanup. It exited zero in 3.835s and matched all 2,074 source files
(`p00-pinned-temp-launch-owned-rpc.json` / `.log`).

Original QML performance discovery requires the immutable legacy comparator
inputs used by `HintCollectorCache.initTestCase()`. They are now exported and
hash-checked against the original FixtureHarness Git-blob oracles in
`p00-pinned-qml-comparator-inputs.json`. Set `FERRIC_TEST_ARTIFACT_DIR` to that
owned directory when running the prescribed QML performance suite. Its
`30e9daf3` comparator is not the site-policy starting baseline: the measured
current candidate must remain the exact `f2c5d6de` production source. The
pinned DPR1 component suite passed all 40 cases in 693.239s with matching
source and comparator hashes. The seven unprofiled collector workloads and
three overlay workloads each retained 10 warm-ups and 100 measured samples.
Nearest-rank p95 and maximum values are in
`p00-pinned-qml-perf-dpr1-summary.json` alongside the raw log and runner record.
At 100% zoom, collector round-trip p95 was 70.824ms for 500 candidates,
122.191ms for 1,000 and 1,007.866ms for 5,000. These measurements use the
existing component fixture and its callback boundary. The overlay component
p95 values at 80%, 100% and 125% zoom were 108.647ms, 94.492ms and 85.047ms.
The production key-to-overlay and passive mutation workloads remain pending.
The original DPR2 component suite passed all 40 cases in 696.640s with the
same pinned source and comparator hashes. The seven unprofiled collector
workloads and three overlay workloads each retained 10 warm-ups and 100
measured samples. `p00-pinned-qml-perf-dpr2-summary.json` records nearest-rank
p95 and maximum values alongside the raw log and runner record. The collector
reported actual WebEngine DPR values of 1.6, 2 and 2.5 at 80%, 100% and 125%
zoom. At 100% zoom, collector round-trip p95 was 69.200ms for 500 candidates,
122.256ms for 1,000 and 1,017.734ms for 5,000. Overlay component p95 at 80%,
100% and 125% zoom was 124.749ms, 97.632ms and 95.396ms. The overlay fixture
reports logical viewport and graphics API without a per-row DPR field.

The original S17 Rust debug-profile benchmark passed in 18.746s with all
2,074 source files matching the pin. Each synthetic workload kept 10 warm-ups
and 100 unprofiled samples for payload decode, label allocation, interaction
setup, text ranking and snapshot. The nearest-rank allocation p95 values for
500, 1,000 and 5,000 candidates were 0.224ms, 0.422ms and 2.171ms. Decode p95
was 9.701ms, 16.681ms and 82.307ms, respectively. These component timings
exclude WebEngine and the overlay. The raw log and runner record are
`p00-pinned-rust-perf-debug.log` and `.json`. The five stages' p95 and max
are in `p00-pinned-rust-perf-debug-summary.json`.

The original optimized S17 benchmark passed in 398.276s including compilation,
with all 2,074 source files matching afterward. The same three workloads and
sample counts produced allocation p95 values of 0.048ms, 0.139ms and 0.697ms,
and decode p95 values of 1.656ms, 3.499ms and 14.049ms. The exact profile and
five stage summaries are in `p00-pinned-rust-perf-release.json`,
`p00-pinned-rust-perf-release-summary.json` and the matching raw `.log`.

`p00-pinned-component-budgets.json` indexes the 26 exact-source component
workloads and their raw log hashes. It applies the frozen Section 11.2 p95
formulas to the corresponding collector and fresh/refresh component boundaries.
The complete browser journeys need their own paired baselines, and these
100-sample rows supply no p99 estimate.

After both Rust source postchecks passed, the v7 measurement patch and the
separate broker diagnostic patch were applied to the detached checkout. The
builder verified the original source manifest plus the eight measurement and
three diagnostic paths before starting Cargo. `p00-v7-build-diagnostic-1/`
records the successful Cargo/CMake build, exact archive and binary hashes,
link command and unchanged source postcheck. CMake recorded the generated
headers from the same Cargo package and linked the selected engine archive.

The DPR 1 passive-refresh smoke reproduced the failure after the page
confirmed all 1,000 mutations. The actual binary, archive and source manifest
matched the diagnostic build record. The trace contains 68 events. Its sole
timeout was Hint `collect-root` operation 22. The original 1,000ms Qt timer
fired at 950.492ms, delivered the timeout, and the renderer callback arrived at
1,105.052ms and was discarded as late. The dirty poll and lease check had
already returned. This localizes the failed operation. The callback interval
includes transport and event-loop delay, so it does not measure script time
in isolation. The first warm-up failed before any measured refresh sample.
See `p00-v7-passive-refresh-diagnostic-1/diagnostic-summary.json`, the raw
`test.log` and `run-metadata.json`.

The full three-file diagnostic patch was reversed before starting the plain
build in `p00-v7-build-plain-1/`. The optimized engine and application harness
built and linked successfully. The source preflight and postcheck matched the
original revision plus the eight approved measurement paths. The serial matrix
in `p00-v7-plain-baselines-1/` uses that receipt for unprofiled passive refresh,
Normal command render and cold Hint render at DPR 1 and 2. The matrix checks
each workload's source manifest and artifact hashes against the build record.

The two unprofiled passive-refresh runs reproduced `frame-timeout` in the
first warm-up at actual DPR 1 and 2. Each receipt confirmed exactly 1,000
mutations and matched the plain build source and artifact hashes. The QtTest
runs exited 1 in 33.344s and 33.340s. Both produced zero measured samples, so
the paired refresh p95 and maximum are unavailable. The 63 marker delegates
in the failure endpoint are from the preceding frame. The availability
receipt is `p00-v7-plain-baselines-1/passive-refresh-availability.json`.

The original Normal colon-to-command render passed both absolute ceilings
with 10 warm-ups and 1,000 measured samples at each DPR. At DPR 1 the
nearest-rank p95/p99/max were 10.243/14.218/17.157ms. At DPR 2 they were
16.810/20.315/25.097ms. This constructed Qt key traverses the production
router to the matching Quick frame swap. The paired no-policy p95 ceilings
are 12.243ms and 18.810ms, with the existing 50ms absolute p99 ceiling.
See `p00-v7-plain-baselines-1/command-render-baselines.json` and the two
raw command result directories.

The DPR 1 cold Hint run finished with a matching build binding and all five
workload rows retained. Each 500-target row rendered 500 markers in all 100
measured samples. The p95/max were 606.586/806.007ms at 80% zoom,
594.200/618.283ms at 100%, and 575.135/607.528ms at 125%. All three exceeded
the existing 200ms p95 ceiling. The 1,000-target row measured 100 samples,
p95 1,096.472ms and max 1,304.462ms, with 63 cluster representatives per
frame. The 5,000-target row timed out in collection on its first warm-up and
produced no percentile. The runner retained four QtTest failures, with
separate complete timing and availability records in
`p00-v7-plain-baselines-1/cold-hint-dpr1-baselines.json`.

At DPR 2, the corresponding 500-visible p95/max were 632.652/726.393ms,
633.307/764.552ms and 635.663/712.656ms. All three exceeded 200ms. The
1,000-target row measured p95 1,090.623ms and max 1,152.054ms over 100 samples
with 63 cluster representatives per frame. The 5,000-target first warm-up
timed out, leaving no valid samples. See
`p00-v7-plain-baselines-1/cold-hint-dpr2-baselines.json`. The six-workload
matrix finished with matching build bindings and final artifact hashes.

The no-Hint idle observation started after the serial cold-render matrix.
`p00-pinned-idle.py` uses the original `xtask perf --suite ui`, the matching
release browser, and the original authorization-gated Quit helper. The
workload is one `about:blank` tab, internal presentation, DPR 1 and offscreen
software rendering. It starts two independent browsers and measures 120
seconds of idle CPU per process before rendered Quit. Resource counters retain
their unavailable-process fields. This measures fresh-process idle behavior.
The lifecycle harness observes retention in the same browser after Hint
cycles. The helper compiled successfully on the first attempt, but the
owned temporary root made the durable-profile Unix socket path too long for
the browser to start. The failed run is retained in
`p00-pinned-idle-before-lifetime/`. The corrected attempt uses `/tmp` for the
original disposable profile, with the PID recorded for cleanup, and a
separate owned XDG runtime directory. It produced two complete 120-second
observations, with rendered Quit confirmed for both. Process-tree CPU was
0.308% and 0.433% of one logical CPU, with six processes and two renderers in
each repetition. PSS was 277,666 and 277,338 KiB. All memory and thread
subjects were available. The wrapper failed while removing the runtime
directory after report generation, as the private bus's document portal was
unmounting. The raw result, original failure and independent report checks
are retained in `p00-pinned-idle-before-lifetime-attempt-2/`. The owned runtime
directory was subsequently removed after the mount had gone.

In the pinned Hint workspace, the 80ms dirty poll runs while `hint_visible`
is true. Closing Hint stops provider tracking and clears pending input. The
lifecycle checkpoints observed `hint_visible` and `poll_pending` false and
the enumerated tracking state empty after close. These source and state
checks cover the ordinary Hint poll. The idle CPU readings are a separate
whole-process observation.

The independent scope review identified two additional ordinary-path workloads.
The v8 test-only patch adds ready text query, timing a native character after
Slash enters text mode and checking rank, delegates and unchanged session and
epoch. It also adds a combined refresh workload with 500 distinct targets and
500 visible delegates. Each batch makes two distinct style writes per target,
for 1,000 writes, and checks the resulting geometry of all 500 targets. The
existing 1,000-target failure records remain separate. The new fixture scripts
passed syntax and synthetic write-count checks. Twelve synthetic parser cases
passed, including duplicate ready rows, missing markers and malformed mutation
receipts (`p00-parser-guard-probe-v8-corrected/summary.json`). The patch applies
to the pinned index and the application harness compiled and linked against
the original engine. The first browser smoke results are below.

The v7 lifecycle smoke completed both established Hint rows, including the
focused foreign frame, with one warm-up and two measured cycles per row.
The two window rows reached an established Hint and failed at `tab-close`,
which the pinned command registry limits to Normal and Command mode. V8
exercises the existing native close event while Hint is live and observes the
ordinary confirmation, view destruction and profile-ledger teardown. The v7
failure is retained in `p00-v7-lifecycle-smoke-1/`. The corrected test-only
overlay built and passed its exact source postcheck in `p00-v8-build-plain-1/`.
All four v8 lifecycle smoke rows passed at DPR 1 with one warm-up and two
cycles each. An independent review identified a stronger check for the first
rendered close confirmation, prepared separately for the full window runs.

The v8 DPR 1 combined refresh smoke also passed. Both recorded cycles retained
all 500 rendered markers, and each receipt reported 1,000 style writes across
500 targets with the expected revision. See `p00-v8-smoke-1/`. These smoke
cycles establish the fixture and production path. The 100-sample baseline and
DPR 2 results remain pending.

The v8 ready smoke completed all four available queued-prefix rows. The eight
500- and 1,000-target prefix and Tab rows reached their expected first input
frame and failed during untimed Backspace reset. The 1,000-target query also
reached its input frame and failed at reset. All three 500-target query rows
recorded text mode, query `z`, active label `aaa` and the exact expected order
of 167 labels, but failed their frame check. V8 did not retain the pre-key
stamp or final displayed marker count, so the trace cannot distinguish a
collection stamp change from delegate or frame readiness. The 5,000-target
rows failed to establish their starting overlay. The v9 test-only patch uses
production Clear outside timing, checks the restored delegates and swap, and
records the query's expected and actual session/epoch and delegate counts.
The pinned index applicability check passed. Its real build and smoke remain
pending.

The v8 activation smoke passed all three 500-target zooms and the 1,000-target
row through fresh proof and an exact click receipt. The 5,000-target row timed
out during Hint preparation.

The DPR 1 rooted render smoke completed 20 of 25 rows with the expected
root/candidate/delegate endpoints and exact build binding. Repeated open
shadow roots, slots, two same-origin frame hops, two focused foreign frame
hops and clipping each passed at 500 targets at all three zooms and at 1,000
targets at 100%. Each 5,000-target shape failed during its first warm-up,
before a usable frame. `p00-v8-smoke-1/roots-dpr1/verified-smoke.json` checks
the full shape/size/zoom matrix, marker counts and fixture facts.

The DPR 1 provider smoke completed 45 of 55 rows. All 20 available collection
rows and 25 fresh-proof rows completed, including the five one-target
controls. Each 5,000-target collection or fresh-proof setup returned
`frame-timeout` at its first warm-up. Those ten rows produced zero samples
and could not confirm cleanup. The fresh rows never reached fresh proof.
`p00-v8-smoke-1/root-stages-dpr1/verified-smoke.json` records the exact
outcomes. Both full combined refresh runs completed with matching source and
artifact receipts. Each used 10 warm-ups and 100 measured samples at 100%
zoom, with 500 visible delegates after all 1,000 writes in every sample.
Nearest-rank p95/max were 743.302/788.026ms at DPR 1 and
885.754/1,001.819ms at DPR 2. The paired no-policy p95 ceilings are
817.632ms and 974.330ms. The verified receipt is
`p00-v8-combined-refresh-1/verified-baselines.json`. The separate 1,000-target
refresh workload remains unavailable. The v9 test-only rebuild started
after both runs, reusing the verified v8 engine archive and browser.

The v9 native harness built with the exact v8 engine archive and browser.
All four lifecycle smoke rows passed again. Every shared and private window
cycle confirmed the initial rendered close prompt, observed view destruction
and returned its profile ledger to baseline. The v9 ready smoke passed all
four available text-query and four available queued-prefix rows. The prefix
and Tab failures recorded a passive replacement session during untimed reset
or an in-flight stamp at input. For example, the 500-target prefix reset
restored all 500 delegates in session 1, then production published session 2
before the swap was observed. The 5,000-target rows remained unavailable.

V10 lets untimed ready setup follow a legitimate replacement session, waits
for an unfiltered frame and rechecks the stamp before timing. Each measured
ready key still requires an unchanged session and epoch. The queued path
still requires a genuine pending key and a newer rendered session. Eight
synthetic event-receipt checks passed and the patch applies to the pinned
index. The v10 native harness built and passed its exact source check, reusing
the v9 engine archive and browser. Its DPR 1 smoke validated all 16 available
rows with the expected starting frame or genuine queued key. The four
5,000-target rows timed out establishing the initial overlay. The completed
smoke verified its artifact hashes in `p00-v10-ready-smoke-1/`. The full
unprofiled ready, rooted render and rooted provider runs at both DPR values
started in `p00-v10-ready-rooted-baselines-1/`.

The full DPR 1 lifecycle run passed all four rows in 336.785s with matching
source and artifact receipts. Ordinary and focused foreign Hint each completed
10 warm-ups and 1,000 established open/close cycles. At all 11 checkpoints per
row, the same renderer documents had zero enumerated elements, observers and
tracked windows after close. Browser and provider tracking state was clear. Qt
descendants, process count, threads and FDs stayed fixed within each row.
Process-tree PSS peaked and then fell, ending 106,284 KiB and 115,751 KiB
above the respective warmed checkpoints. The measurements do not identify
the allocations behind that growth.

Shared and private window teardown each completed 10 warm-ups and 100 cycles.
Every cycle confirmed the rendered close prompt, destroyed the tracked view
and restored the profile ledger. Each closed host was still present and hidden
at the immediate release observation. From warm-up to cycle 100, Qt windows
rose from 11 to 111 and renderers from 12 to 112 in each row. FDs rose by
3,800 in each row. Process-tree PSS rose from 683,336 to 4,175,116 KiB in the
shared row and from 3,014,925 to 4,184,608 KiB in the private row. The source
close path releases registered consumers and closes the window. Independent
review matched all 44 checkpoints and 220 immediate window outcomes to the
nested records. Every earlier renderer PID and start-time pair was present at
each later checkpoint in its window row. A separate check after the test
process exited found none of the 232 sampled process identities still present.
The snapshots establish retention while the browser was running. They leave
reclamation in a long-running browser and retaining ownership unresolved.
Keep this pre-existing growth as a
remediation and qualification subject for P15-07. Exact checkpoints and
outcomes are in `p00-v9-full-lifetime-activation-1/lifetime-dpr1/verified-lifetime.json`.

The full DPR 1 activation run completed 10 warm-ups and 100 measured samples
for each 500-target zoom and for 1,000 targets at 100%. Nearest-rank p95/max
were 177.411/247.141ms at 80%, 188.990/261.514ms at 100%, and
159.952/229.087ms at 125%. At 1,000 targets they were 486.212/675.537ms.
Timing starts at constructed Enter on the selected Hint and ends at the
callback that confirms the exact DOM click. It includes production fresh
proof, action and receipt polling. The 5,000-target row timed out during
preparation before activation, leaving no valid samples. The source and
artifact binding and per-event click receipts verified. See
`p00-v9-full-lifetime-activation-1/activation-dpr1/verified-baselines.json`.

DPR 2 completed the same four available workloads. The 500-target p95/max
were 168.305/189.621ms at 80%, 151.210/161.275ms at 100%, and
159.350/174.793ms at 125%. At 1,000 targets they were 489.400/678.098ms.
The 5,000-target first preparation again returned `frame-timeout`. The
complete matrix verified its source and artifact bindings and its final
artifact hashes. See `activation-dpr2/verified-baselines.json` under the
same run directory. The click receipts observe ordinary semantic activation,
whose `isTrusted` was false. Native delivery and trust qualification belong
to the later native tests.

Independent review recomputed all eight completed activation p95/max pairs
and matched every one of the 110 ordered receipts per row to its exact
`target-0` / `Target 0` identity and click count. The saved source, producer,
runtime and summary hashes matched. Both unavailable 5,000-target rows retain
null summary percentiles and ceilings.

The full DPR 1 ready-input run completed all 16 available rows, each with
10 warm-ups and 100 measured samples. Every established-session event matched
its starting session/epoch; every queued-prefix event proved a pending key
with no active Rust session and a newer rendered session/epoch. Each
500-target established starting frame had all 500 markers. The natural
filtered counts were 81 for prefix and 167 for query; Tab kept 500.

| Operation | 500 at 80% p95/max ms | 500 at 100% p95/max ms | 500 at 125% p95/max ms | 1,000 at 100% p95/max ms |
| --- | --- | --- | --- | --- |
| Ready prefix | 93.670/136.774 | 89.917/90.786 | 89.306/123.876 | 72.082/95.345 |
| Tab selection | 66.341/86.280 | 59.292/69.578 | 59.683/64.074 | 63.490/97.823 |
| Text query | 126.057/145.972 | 123.126/158.797 | 123.175/159.553 | 50.024/62.528 |
| Queued prefix | 635.461/754.096 | 537.686/573.491 | 547.573/631.611 | 959.431/974.322 |

The queued interval starts at the genuinely pending key and includes remaining
collection and replay. The earlier Hint invocation is outside that interval.
All four 5,000-target rows failed initial untimed preparation with zero valid
samples; their percentiles and reference ceilings remain null. The run exits
4 because of those availability failures. Exact source/archive/binary binding,
all sample receipts and recomputed nearest-rank summaries are verified in
`p00-v10-ready-rooted-baselines-1/ready-dpr1/verified-baselines.json`.
DPR 2 also completed all 16 available rows, each with 10 warm-ups and 100
measured samples. Its four 5,000-target rows failed untimed initial preparation
with zero samples and null percentiles. The workload exits 4; the complete
event matrix, raw percentiles and exact build binding verify in
`p00-v10-ready-rooted-baselines-1/ready-dpr2/verified-baselines.json`.

| Operation | 500 at 80% p95/max ms | 500 at 100% p95/max ms | 500 at 125% p95/max ms | 1,000 at 100% p95/max ms |
| --- | --- | --- | --- | --- |
| Ready prefix | 101.594/129.125 | 97.482/133.459 | 97.296/123.385 | 86.094/106.204 |
| Tab selection | 80.247/104.243 | 79.756/128.710 | 79.532/120.812 | 71.600/77.500 |
| Text query | 142.681/176.033 | 134.995/166.365 | 142.061/253.789 | 58.242/70.585 |
| Queued prefix | 598.345/679.725 | 596.143/648.461 | 593.023/694.167 | 1,138.969/1,188.022 |

The same established-session and queued-input checks apply. These are
constructed native keys through the production router to a matching Qt Quick
frame, excluding compositor presentation. The initial full rooted render
attempt at DPR 1 was subsequently stopped for the logging failure below.
Independent DPR 2 review matched all 1,760 events, exact p95/max/reference
ceilings and every source/producer/runtime/result/log hash binding. It found
no issue in the stated constructed-input-to-Quick-frame scope. The four
unavailable rows are the only failures behind exit 4.

In the stopped v10 attempt, the DPR 1 1,000-target slot row stopped at measured
entry iteration 89 after 10 valid warm-ups and 79
valid measured samples. The log records `frame-timeout`; the failed event has
elapsed -1, zero rendered markers and zero leases. Its 79 retained samples
and fixture/event accounting verify, while its summary percentiles and
reference ceilings remain null. Independent review matched all 90 events:
indices 0–88 reached 63 cluster markers, while attempt 89 accepted the command
but failed the endpoint assertion before sample append. It confirmed the
failure cutoff and exact retained timings, without inferring the underlying
cause. Keep this partial result rather than treating progression to the next
row as completion. Terminal whole-run source/artifact checks and exit review
remain pending.

The corrected v11 attempt has since reported all 10 warm-ups and 100 measured
samples for that same slot row. Its p95/max are 645.267/720.068 ms, and all 110
events match the row's timings, fixture facts and endpoints. The complete
sample set supplies paired p95 references of 709.794 ms for no policy and
806.584 ms for matched collection. The terminal DPR 1 receipt below now confirms the row and
run build binding; the whole-matrix artifact postcheck remains pending. Preserve the earlier timeout as a separate
availability finding; the logging correction establishes no fix for its cause.

### High-volume logging correction — 11 November 2026

The v10 DPR 1 rooted process reached QtTest's message cap immediately after
the clipping/500/100% event at iteration 29. QtTest continued running while
suppressing required QINFO sample and result records. The owned matrix was
stopped through its signal handler; its runner retired the test process group
and completed its source postcheck. The interrupted attempt retains 21 rows
and 1,884 events in `p00-v10-ready-rooted-baselines-1/roots-dpr1/` plus
`qt-log-limit-stop.json`. It cannot supply a complete rooted baseline. Every
observed process from that owned tree was retired.

The v11 test-driver correction invokes `-maxwarnings 0` and treats the
suppression sentinel as an explicit harness error, including when the child
exits 0. The installed Qt 6.11.2 help and the
[QtTest command-line documentation](https://doc.qt.io/qt-6/qtest-overview.html)
identify this option. An actual qualified-Qt probe retained 2,001 of 2,501
records with the default limit and all 2,501 with the corrected argument;
both probe children exited 0. Its source, logs and verification are under
`p00-qt-log-limit-probe/`. Three synthetic driver checks also passed in
`p00-v11-logging-guard-probe.json`.

Independent review confirmed that the runner is the sole changed file among
the eight v10/v11 measurement paths. C++, fixtures, sample counts, timeouts and
endpoints are preserved. The v11 patch SHA-256 is
`8f3669d66194541798f047f2ca1f6feba99d5a4277d890bf6be86a26424ed08d`.
`p00-v11-build-plain-1/build-record.json` passed its source postcheck using the
verified v10 producer; its native binary, archive and browser hashes are
identical. Review found no suppression sentinel in the completed earlier
measurement logs, so their evidence remains retained. The corrected four-run
rooted matrix has started in `p00-v11-rooted-baselines-1/`, with explicit
unlimited logging in the runtime command. It preserves the stopped attempt.

### DPR 1 rooted render — full corrected run

The corrected run retained all 25 rows and 2,205 sample events. Each of the
20 available rows completed 10 warm-ups and 100 measured samples. All five
5,000-target shapes failed their first warm-up before a usable frame, with
zero accepted samples and null percentiles/reference ceilings.

| Shape | 500 at 80% p95/max ms | 500 at 100% p95/max ms | 500 at 125% p95/max ms | 1,000 at 100% p95/max ms |
| --- | --- | --- | --- | --- |
| Two shadow levels per item | 494.297/540.799 | 487.621/545.764 | 500.410/536.119 | 680.157/1,263.410 |
| Slots | 474.842/528.108 | 482.675/546.648 | 533.232/673.414 | 645.267/720.068 |
| Two same-origin frame hops | 521.364/1,119.972 | 501.442/544.317 | 535.283/762.845 | 733.966/795.412 |
| Two focused foreign hops | 810.971/1,140.890 | 812.655/859.503 | 751.188/772.243 | 1,794.867/1,873.081 |
| Clipping | 498.519/554.793 | 457.779/495.433 | 471.604/484.863 | 656.669/681.546 |

Every completed 500-target endpoint retained all 500 markers. All fifteen
500-target rows exceeded the existing 200 ms p95 ceiling; their no-policy and
matched references remain capped at 200 ms. Each 1,000-target row completed
100 samples; its exact paired references and ordinary clustered marker counts
are retained in `p00-v11-rooted-baselines-1/roots-dpr1/rooted-audit.json`.

The measurement starts at the public `hint buttons` command and ends at the
matching Quick frame. It excludes compositor presentation and establishes no
native-key or physical-input qualification. Exit 20 matches the fifteen
threshold failures and five availability failures. Source/build binding and
source postcheck match; logging suppression and parse errors are absent.

Independent review reproduced the raw log rows/events, fixture facts, frame
paths, accepted sample arrays, p95/max and frozen references. Foreign rows
retained three leases for their two-hop owner chain. Runtime, manifest and
small-file hash bindings match. The DPR 2 results, both provider runs and the
matching final source/artifact postcheck are recorded below and included in
the completed P00 exit review.

### DPR 2 rooted render — full corrected run

DPR 2 also retained all 25 rows and 2,205 events. Twenty available rows each
completed 10 warm-ups and 100 samples. All fifteen 500-target rows exceeded
200 ms; all five 5,000-target first warm-ups failed with zero accepted samples
and null percentiles/reference ceilings.

| Shape | 500 at 80% p95/max ms | 500 at 100% p95/max ms | 500 at 125% p95/max ms | 1,000 at 100% p95/max ms |
| --- | --- | --- | --- | --- |
| Two shadow levels per item | 515.796/567.688 | 516.205/567.936 | 553.882/631.353 | 689.760/1,223.748 |
| Slots | 506.298/575.508 | 482.381/565.041 | 487.701/531.247 | 656.557/1,127.985 |
| Two same-origin frame hops | 571.844/629.915 | 536.049/553.144 | 575.092/800.312 | 747.991/821.090 |
| Two focused foreign hops | 827.184/861.325 | 931.423/1,061.653 | 781.770/792.396 | 1,666.092/1,732.870 |
| Clipping | 477.734/498.754 | 478.471/539.135 | 512.233/698.180 | 649.368/742.883 |

The exact reference ceilings and marker counts are in
`p00-v11-rooted-baselines-1/roots-dpr2/rooted-audit.json`. Independent review
matched the complete raw log, fixture facts, frame paths, warm-up/sample
accounting, p95/max and all reference values. All completed 500-target
endpoints retained 500 markers, and foreign events retained three leases.
Runtime/build/source bindings match; exit 20 matches the twenty Qt failures.
There is no logging suppression, parse error or harness error.

Both completed render runs retain 4,000 measured samples and 4,410 total
events, including their ten failed 5,000-target warm-ups. This records original
performance and availability failures. Both provider runs subsequently completed
and are independently reviewed below; the final source/artifact postcheck also
matches. These records are included in the completed P00 exit review.

### Provider components — observations recorded during DPR 1

Provider DPR 1 partial observation while the run is active: the two-shadow-level,
1,000-target collection-only row stopped at iteration 80 after 10 valid
warm-ups and 70 measured samples. Its failed callback arrived once with
`frame-timeout`, zero candidates and elapsed 1,022.813117 ms. The assertion
failed before collection-sample append. The 81-event sequence and retained
timings verify; p95/max/reference ceilings remain null. Cleanup reports one
failed acknowledgement, which leaves teardown unconfirmed for that attempt.
This establishes neither a retention cause nor a fix. The independent fresh
row follows on a new fixture document. Independent review matched all 81
events and the exact 70-entry timing/inspection/fixture/truncation arrays,
confirmed the exclusion before append and the unsuccessful cleanup
acknowledgement, and found no basis to identify its cause or infer retained
resources. Whole-run source/artifact checks remain pending.

The 1,000-target slot collection-only row later stopped at iteration 88 after
10 warm-ups and 78 measured samples. Its failed event reports one callback,
`frame-timeout`, zero candidates and elapsed 951.136010 ms; cleanup records
one failed acknowledgement. The 89-event sequence and retained sample arrays
pass the verifier, with null percentiles and reference ceilings. Keep this
second partial collection result alongside the independent fresh row; final
dataset review and whole-run binding remain pending.

That independent 1,000-target slot fresh row also retains a partial result:
77 validated fresh samples and 77 collection-setup samples after 10 warm-ups.
Collection setup at iteration 87 then returned `frame-timeout` with zero
candidates, before a fresh request was issued for that iteration. Its 175
ordered events verify, excluding that failed setup from the arrays; cleanup
reports one failed acknowledgement. The row's final availability describes
the failed attempt, while the earlier fresh samples remain preserved. Its
percentiles and reference ceilings remain null.

Independent review confirmed both slot cutoffs and exact retained arrays:
collection events end at 88, while the fresh case has setup events through 87
and fresh events only through 86. The failed iteration's availability field
does not discard the earlier 77 validated proof samples. No whole-run or
resource-retention conclusion follows before the terminal checks.

### Rooted provider — DPR 1 terminal results

The corrected v11 DPR 1 provider run is terminal and independently reviewed.
`p00-v11-rooted-baselines-1/root-stages-dpr1/rooted-audit.json` accounts for all
55 rows and 7,589 events: 40 full rows, five positive partials and ten
zero-sample 5,000-target failures. Full rows retain 10 warm-ups and 100 measured
samples. There are 4,407 accepted primary samples, including 407 from partial
rows; partial rows have no percentile or reference ceiling. Runtime and child
exit 15 match the fifteen Qt failures. Source postcheck and per-run build
binding match; there is no logging suppression, parse error, harness error or
harness-level timeout. The final source/artifact postcheck subsequently matched;
its receipt is recorded with the DPR 2 terminal results below.

These component intervals run from the unmodified production provider request
to its native callback observation and include transport. Collection setup on
fresh rows is preserved separately; the independent collect rows own the
collection baseline. These intervals do not qualify keyboard input, usable
Hint rendering, activation or native delivery. Full-row p95/max values below
use milliseconds with no outlier removal.

| Shape | Metric | 500 at 80% | 500 at 100% | 500 at 125% | 1,000 at 100% |
| --- | --- | --- | --- | --- | --- |
| Two shadow levels per item | collect | 177.325/214.430 | 165.552/253.048 | 190.793/236.589 | Partial (70/100) |
| Two shadow levels per item | fresh | 10.238/11.552 | 10.147/14.611 | 10.629/15.391 | 9.999/12.275 |
| Slots | collect | 167.397/200.104 | 164.625/205.731 | 160.511/215.302 | Partial (78/100) |
| Slots | fresh | 10.030/11.357 | 10.349/11.614 | 10.524/12.363 | Partial (77/100) |
| Two same-origin frame hops | collect | 196.853/244.130 | 190.096/223.690 | 212.933/227.060 | 503.534/562.533 |
| Two same-origin frame hops | fresh | 11.433/17.201 | 11.660/14.772 | 11.132/13.978 | 7.281/12.282 |
| Two focused foreign hops | collect | 202.457/218.334 | 192.768/212.291 | 205.911/266.145 | 493.277/564.094 |
| Two focused foreign hops | fresh | 29.677/523.897 | 28.281/32.109 | 29.052/205.886 | 28.896/83.189 |
| Clipping | collect | 142.109/171.681 | 138.132/187.872 | 145.754/202.698 | Partial (92/100) |
| Clipping | fresh | 10.439/12.122 | 7.318/11.689 | 10.641/11.427 | Partial (90/100) |

All five independent one-target fresh controls completed 100 samples. They
provide a separate small-fixture reference and do not qualify fresh proof at
5,000 targets:

| Shape | One-target fresh p95/max ms |
| --- | --- |
| Two shadow levels per item | 9.730/10.969 |
| Slots | 5.845/12.392 |
| Two same-origin frame hops | 7.137/12.653 |
| Two focused foreign hops | 26.376/27.516 |
| Clipping | 9.838/22.218 |

The clipping collection partial retains 92 measured samples before timeout at
iteration 102 (1,051.825405 ms). The independent clipping fresh partial retains
90 fresh and 90 collection-setup samples; setup at iteration 100 returned one
`frame-timeout` callback with zero candidates in 985.327808 ms, before any fresh
request for that iteration. Their 103 and 201 events respectively verify;
both report successful cleanup acknowledgement. The earlier shadow/slot
partials and all ten 5,000-target failures retain unsuccessful cleanup
acknowledgements. No resource-retention cause is inferred from these outcomes.

Foreign fresh maxima of 523.896919 ms at 80% zoom and 205.885860 ms at 125%
remain in the data. The receipt retains exact per-workload Section 11
references for completed rows and null references for partial/unavailable
rows. Independent review matched raw log exports, event ordering, fixture
facts, candidate identity/geometry, sample cutoffs, statistics and bindings.
DPR 2 terminal results follow below. Both receipts are included in the
completed independent P00 exit review.

### Provider DPR 2 — observations recorded during the run

The DPR 2 run retains a partial 1,000-target slot fresh result in
`p00-v11-rooted-baselines-1/root-stages-dpr2/test.log`: 77 measured fresh and
77 collection-setup samples after 10 warm-ups. Setup iteration 87 returned one
`frame-timeout` callback with zero candidates in 1,051.113131 ms. No fresh
request was issued for that iteration. Independent review matched all 175
ordered events (setup 0–87, fresh 0–86), exact retained arrays and exclusion
of the failed setup. Cleanup records one unsuccessful acknowledgement;
percentiles and reference ceilings remain null. The availability field
describes the failed iteration and does not erase earlier valid proofs.
No resource cause or terminal source/artifact conclusion follows from this
partial observation. At that observation, DPR 2 and the whole-matrix postcheck
remained pending; terminal evidence follows below.

The independent 1,000-target focused-foreign collection row also retains a
partial result: 66 measured samples after 10 warm-ups, before iteration 76
returned one `frame-timeout` callback with zero candidates in 1,042.791509 ms.
Independent review matched all 77 events (0–76) and the exact 66-entry timing,
inspection, fixture and truncation arrays. The failed event is excluded before
append. Cleanup records successful guarded acknowledgements; this establishes
no resource-retention or timeout cause. Percentiles and references remain null.

The independent focused-foreign fresh row subsequently retained 80 fresh and
80 collection-setup samples after 10 warm-ups. Setup iteration 90 returned one
`frame-timeout` callback with zero candidates in 1,149.220480 ms; fresh 90 was
never requested. Independent review matched its 181 ordered events (setup
0–90, fresh 0–89) and exact arrays, excluding the failed setup. Cleanup records
successful guarded acknowledgements; percentiles and references remain null.
The availability field describes the failed iteration. Terminal source/artifact
checks and the whole-matrix postcheck were still pending at this observation.

### Rooted provider — DPR 2 terminal results and final matrix checks

The corrected v11 DPR 2 provider run is terminal and independently reviewed.
`p00-v11-rooted-baselines-1/root-stages-dpr2/rooted-audit.json` accounts for all
55 rows and 7,372 events: 40 full rows, five positive partials and ten
zero-sample 5,000-target failures. Full rows retain 10 warm-ups and 100 measured
samples. There are 4,284 accepted primary samples, including 284 from partial
rows. Runtime and child exit 15 match the fifteen Qt failures. There is no
logging suppression, parse error, harness error or harness-level timeout.
All five one-target controls completed independently; they do not qualify
fresh proof at 5,000 targets.

The measurement boundary and nearest-rank method match the DPR 1 component
record above. Collection setup remains separate from fresh primary latency.
Full-row p95/max values below use milliseconds without outlier removal;
partial rows have no percentile or reference ceiling.

| Shape | Metric | 500 at 80% | 500 at 100% | 500 at 125% | 1,000 at 100% |
| --- | --- | --- | --- | --- | --- |
| Two shadow levels per item | collect | 190.009/274.175 | 167.981/235.131 | 180.332/227.872 | 508.882/920.291 |
| Two shadow levels per item | fresh | 10.213/11.436 | 6.625/11.419 | 9.983/13.205 | 7.297/10.847 |
| Slots | collect | 156.146/213.360 | 155.102/199.537 | 161.925/197.737 | 444.351/836.809 |
| Slots | fresh | 10.164/14.876 | 7.924/13.167 | 10.209/14.288 | Partial (77/100) |
| Two same-origin frame hops | collect | 226.094/526.423 | 188.429/233.576 | 251.527/388.210 | 511.427/772.368 |
| Two same-origin frame hops | fresh | 11.128/12.194 | 11.269/16.646 | 11.276/12.835 | 8.042/13.614 |
| Two focused foreign hops | collect | 212.143/249.311 | 220.212/256.601 | 211.224/270.994 | Partial (66/100) |
| Two focused foreign hops | fresh | 29.429/177.712 | 28.871/34.081 | 28.833/176.751 | Partial (80/100) |
| Clipping | collect | 286.916/322.890 | 398.044/780.983 | 331.163/400.598 | Partial (45/100) |
| Clipping | fresh | 15.058/18.927 | 15.780/20.896 | 18.032/30.199 | Partial (16/100) |

| Shape | One-target fresh p95/max ms |
| --- | --- |
| Two shadow levels per item | 9.860/10.555 |
| Slots | 5.998/10.534 |
| Two same-origin frame hops | 11.219/13.026 |
| Two focused foreign hops | 49.632/54.087 |
| Clipping | 17.251/33.398 |

The terminal review confirmed all five partial cutoffs: slot fresh 77 samples
before setup 87; foreign collection 66 before 76; foreign fresh 80 before
setup 90; clipping collection 45 before 55; clipping fresh 16 before setup 26.
Failed operations are excluded and no subsequent fresh event is invented.
Slot fresh and all ten 5,000-target failures retain unsuccessful cleanup
acknowledgements; the four foreign/clipping partials retain successful guarded
acknowledgements. These outcomes establish no resource-retention cause.
All complete statistics and frozen references recompute exactly, with null
references for partial/unavailable rows. Raw logs, exports, fixture facts,
geometry and source/build/runtime bindings passed independent review.

Both provider runs retain 8,691 accepted primary samples and 14,961 events.
The complete corrected matrix is terminal as `failed-workloads`, preserving
all threshold and availability failures in its four workloads. Every runtime
binding matches and its final artifact postcheck is true. The separate
`p00-v11-final-postcheck.json` independently rehashed the current 2,079-file
original-plus-overlay source inventory, application test binary, engine
archive and browser executable; all match the selected v11 build. It also
checks all four terminal runtime bindings and retains the exact matrix hash.
No baseline failure has been reclassified as a performance pass. Independent
exit review approved closure of P00-11 and P00-GATE within baseline and test
preparation scope.

### Browser baseline reproduction

From the repository root, the test-only v11 inputs are enumerated in
`tools/performance/p00/p00-measurement-overlay-v11.json`. The matching
patch applies to the original detached revision. The build runner verifies
every file in that worktree against the original source manifest plus the
eight test paths before invoking the pinned Qt 6.11.2 wrapper. Its build
receipt identifies the reused Cargo archive and browser, the CMake link
command and the exact application test binary. The retained scripts,
original source manifest and exact overlay patches are under
[`tools/performance/p00`](../../tools/performance/p00/README.md). That README
documents a fresh artifact directory, original-source checks, comparator
exports, build and idle prerequisites. Generated evidence remains under
`target/site-policy-baseline`; historical receipts retain the hashes of the
ignored scripts actually used for those runs.

The serial unprofiled matrix uses the following command. Each output directory
must be new. `--smoke` selects two observed cycles instead of the full sample
counts and is used separately from this matrix.

```sh
python3 tools/performance/p00/p00-run-plain-matrix.py \
  --build target/site-policy-baseline/p00-v11-build-plain-1/build-record.json \
  --output-dir target/site-policy-baseline/p00-v11-rooted-reproduction \
  roots:1 roots:2 root-stages:1 root-stages:2
```

The matrix records the exact command, return code and source and binary
binding for each workload. A workload failure leaves its raw samples and
failure stage available and continues the independent workloads. An invalid
source or artifact binding stops the matrix. Each workload directory contains
the runtime metadata, complete test log, parsed rows and per-sample events.

The retained rooted verifier now cross-checks those events against each row,
including collection-before-fresh order, exact warm-up/iteration identities,
typed fixture facts, accepted callback endpoints and failure-stage cutoffs.
Its terminal summary hashes the raw event file. This preserves valid
collections when later fresh or cleanup fails, while excluding the failed
operation from successful samples. All 80 historical rooted smoke rows and
285 events pass; 18 mutated-record negatives are rejected and two partial
sample-preservation cases pass (`p00-root-event-verification.json`). Independent
read-only review found no remaining material issue. The live timing inputs
and original production source are unchanged. Both rooted-render and both
provider summaries are independently reviewed. The final source/artifact
postcheck matches; full P00 exit review approved the retained evidence and
reproduction scope.

The main application measurement translation unit and three fixtures now
match v10, with its frame timestamp/member and slot declarations merged into
the shared harness. The existing later-phase test registrations are preserved.
The main runner permits candidate production changes by default and offers
`--require-baseline`; the immutable detached runner continues to enforce the
original production source. Seven synthetic guard control-flow cases passed
(`p00-main-runner-guard-probe.json`), including diagnostic smoke and missing
historical Git objects. These use a mocked producer and do not qualify the
main checkout's C++ build or runtime. Syntax/path checks and all eight
plain/diagnostic v7–v10 expected inventories match the historical drivers.
Independent read-only review confirmed the minimal merge, exact retained
patches, path handling and candidate/strict guard semantics. Its fresh-root
idle findings were fixed: the original runner now has a `build-tools` stage,
and idle validates that passing original-source receipt and its exact `xtask`
hash. Nine synthetic tool-receipt checks passed
(`p00-idle-tools-guard-probe.json`), including failed/mismatched source receipts,
boolean/nonzero exit codes and changed binary hashes. The reviewer found no
remaining source-level reproduction blocker and independently matched all
1,760 DPR 1 ready events and all summary values to the raw receipts. Fresh-root
execution remains unverified. During the logging-fix pause, the merged main
C++ harness compiled successfully in 138.664 seconds with an unchanged source
postcheck and production archive/browser hashes
(`p00-main-measurement-build-1/build-record.json`). The main command smoke then
passed three QtTest cases, one warm-up and two measured samples, with exact
source postcheck and no percentile claim
(`p00-main-measurement-command-smoke-1/run-metadata.json`). This verifies the
merged harness build and basic candidate command journey; later-phase
production and native-platform qualification remain held.
Independent review confirmed the shared main build/smoke source manifest and
binary hash, runtime flags and narrow smoke scope, together with the v11
2079-file source binding and logging probes. No blocker was found for starting
the corrected rooted matrix.

The independent P00 coverage review identified the rooted matrix and main build
as the final original-source workload gaps. The main build, command smoke and
corrected rooted matrix are now recorded. Independent evidence review verified
every terminal summary and artifact binding, indexed the frozen ceilings and
preserved unavailable rows and the adverse window-resource trend. The complete
P00 item-by-item exit review approved closure on 11 November 2026. Many-rule,
new semantic-policy, remap and capture/draft workloads belong
to their later implementation phases; their controls and ownership are already
inventoried.

### P00 exit review — 11 November 2026

The independent item-by-item reviewer found no material missing or contradicted
P00 requirement. It checked the 2,288-case compatibility oracle, controls and
ownership contracts, source inventory, required fixtures and executed tests,
all 112 acceptance IDs, unqualified native matrix, prescribed command outcomes
and reproduction package. It independently verified all 2,079 current source
files and all four terminal runtime hashes, settings and build bindings.

The separate performance/reproducibility reviewer verified final tables and
combined counts, frozen references, source/artifact identities, measurement
boundaries and preserved failures. Three stale status phrases were corrected.
Both reviews support closing P00-11 and P00-GATE. The 200 ms failures,
availability timeouts, resource-retention growth and platform prerequisites
remain unresolved for their owning implementation/qualification phases.
P01 feasibility is the next work; preserved P02/P03 changes remain held for
revalidation after its independent review. This is no claim of native,
performance or complete feature qualification.

Starting revision: `f2c5d6deebd97eb861084503f25613a6a819949d`, identical to
the implementation plan's pinned baseline. Initial checkout clean on main;
implementation branch: `feature/site-interaction-policy`. The owner plan is
`../Ferric_Site_Policy_Advanced_Hints_Implementation_Plan.md` from repository
root. It remains the single execution checklist; this note retains baseline
evidence and does not replace its scope.

## Environment

- Host: thinkpad, Linux x86_64 `7.2.11-arch1-1`, Intel i5-7200U, four logical CPUs.
- rustc 1.98.0 (`88d9e12ae`), Cargo 1.98.0, GCC 16.2.1.
- Desktop reports Wayland, `WAYLAND_DISPLAY=wayland-1`, X display `:0`.
  Actual compositor input, layouts, lock state and X11 qualification not yet run.
- Default `/usr/bin/qmake6` reports **6.12.0**, which is not the qualified SDK.
- Qualified command selection: `tools/performance/with_qt611.sh COMMAND`.
  Its SDK at `~/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64`
  reports 6.11.2 and contains matching
  `include/QtGui/6.11.2/QtGui/qpa/qplatformwindow_p.h`.
  Runtime fingerprint from rebuilt QML runner: Qt/WebEngine 6.11.2, Chromium
  140.0.7339.225, security patch 151.0.7922.71, QPA offscreen.
  Desktop Hyprland 0.56.2 (`efb50993780079460b0cbed1363e2166a2de1d9f`);
  active keyboards report gb/English (UK), CapsLock off, NumLock on.
  Full SDK provenance and owned compositor remain to verify.
- No host packages, desktop configuration or real browser profiles changed.

## Executed baseline checks

Logs are retained locally under ignored `target/site-policy-baseline/`.
Commands below ran from repository root against unchanged production source.

| Command | Observed result | Evidence |
| --- | --- | --- |
| `cargo test --locked -p ferric-browser-core -p ferric-browser-config -p ferric-browser-ipc` | Exit 0; config 69, core 178, IPC 40 tests passed; no failures/ignored tests | `core-config-ipc.log` |
| `cargo fmt --all -- --check` | Exit 0 | `fmt.log` |
| `python3 tools/qualification/kde_free.py policy` | Exit 0, policy passed | `dependency-policy.log` |
| `tools/performance/with_qt611.sh cmake -S crates/ferric-browser-engine-qt/tests/native-insert -B target/site-policy-baseline/native` | Exit 0; optional TaskTree/Vulkan discovery warnings retained | `native-configure.log` |
| `tools/performance/with_qt611.sh cmake --build target/site-policy-baseline/native --parallel 2` | Exit 0 | `native-build.log` |
| `tools/performance/with_qt611.sh ctest --test-dir target/site-policy-baseline/native --output-on-failure --output-junit ../native-results.xml` | Exit 0; native_insert passed, 20.43 seconds | `native-test.log`, `native-results.xml` |

Native evidence above is offscreen/software Qt-generated event evidence only.
It establishes neither remapping nor physical/compositor/layout qualification.
Production `cargo build --locked` under the SDK wrapper passed (18m34s). Full workspace/application/engine,
lint/check and performance baseline gates have not run.

## Scaffolding and next checks

Source contracts are inventoried in [site-policy-p00-contracts.md](site-policy-p00-contracts.md).
The native matrix is [site-policy-native-matrix.json](site-policy-native-matrix.json);
every Cartesian-product cell starts Not run. No generated matrix row proves support.
The new `tests/fixtures/site-policy/interaction.html` under the engine crate
provides explicit navigation, distinct same-URL cards, a tall primary, repeated
shadow primaries, editable receivers, adversarial mutation controls and a
bounded 4,096-entry native-event observer. It is exercised by `tests/qml/tst_site_policy_baseline.qml`: four cases
passed (six including setup/cleanup), 0 failed/skipped, 2.814s.
`roots.html` adds direct slots, same-origin frame/shadow and occlusion; slot
mutation tests recollect and assert discovery count changes. Trace structure
checks use explicitly synthetic, untrusted events and do not qualify native delivery.
Evidence: `fixture-tests.log` and `runtime-fingerprint.log`.
Existing `tst_webapp_interaction.qml` baseline passed 50 cases, 0 failed/skipped,
34.620s, including 1,000-cycle cleanup with zero retained elements/observers/windows;
evidence: `webapp-interaction.log`. It ran offscreen with exposed GC.
The parser now has a 2,288-cell compatibility oracle and boundary tests; these
passed after the production build; see `parser-matrix.log`.
All 112 mandatory acceptance rows are transcribed in `site-policy-acceptance.json`;
empty tests/evidence lists remain explicit gaps.

Performance gates are frozen to plan Section 11.2 before feature edits:
no-policy p95 must not regress by both >10% and >2ms; matched p95 ceiling is
paired ordinary baseline + max(10ms,25%); p99 ceiling is baseline +
max(20ms,35%) subject to stricter existing absolute ceilings. Native added p99
routing <=1ms. Preserve all existing stricter thresholds. No timing results
have been measured yet; baseline per-workload absolute values remain to collect.

No P00 item requiring runtime coverage, exhaustive executable tables, complete
acceptance mapping or performance evidence is marked complete. Review findings
and follow-up evidence must be retained before closing P00.

Interim review checked parser oracle, fixture tests and runtime fingerprint
instrumentation; no incorrect expectations found. Slot freshness assertions
were added and passed after review. Complete P00 review remains outstanding.
The production build and targeted parser matrix are complete. Performance
samples wait until the remaining compilation is quiescent.

## Follow-up baseline execution

- Full QML suite (`QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=
  QT_QUICK_BACKEND=software QTWEBENGINE_CHROMIUM_FLAGS="--disable-gpu --js-flags=--expose-gc"
  tools/performance/with_qt611.sh target/site-policy-baseline/qml/qml_tests
  -input crates/ferric-browser-engine-qt/tests/qml`): **1,237 passed,
  zero failed/skipped**, 181.589 seconds, exit 0; `qml-suite.log`.
- Added `sitePolicyBaselineNativeTrace` to the production router suite:
  once/persistent cases passed, four including setup/cleanup. Constructed Qt
  input observed one p/KeyP down/up pair, trusted=true, activation=true;
  zero click/submit. These observations do not qualify transformed or real
  compositor input. Native fixture also records logical text and input data.
  Exact command: `QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
  QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu tools/performance/with_qt611.sh
  target/site-policy-baseline/native/native_insert sitePolicyBaselineNativeTrace`.
  Evidence: `native-trace-tests.log`.
- Full native suite rebuilt and passed again after the new tests, exit 0,
  20.91 seconds: `native-trace-suite.log`, `native-trace-results.xml`.
  Command: `tools/performance/with_qt611.sh ctest --test-dir
  target/site-policy-baseline/native --output-on-failure --output-junit
  ../native-trace-results.xml`.
- Independent interim review found no substantive defect in the native test
  or its CMake fixture wiring; ASCII unchanged-passthrough scope is explicit.
- Configuration source/publication inventory extended in the contracts note.
  All acceptance rows now have proposed pure/integration test owners; these
  are deliberately labelled planned, not implemented or executed.

Pending processes at this checkpoint: production `cargo build --locked`
(session 10622) is actively compiling engine C++; the parser compatibility
command `tools/performance/with_qt611.sh cargo test --locked
-p ferric-browser-engine-qt --lib site_policy_baseline_` is queued on Cargo's
build-directory lock (session 82385; `parser-matrix.log`). Poll these handles
before starting replacement builds. Constructor inventory retained with
`rg -n 'SiteRule \{' crates --glob '*.rs'` in
`site-rule-constructors.txt`: 19 occurrences including the type definition.

## Schema contracts before implementation

`cargo test --locked -p ferric-browser-config --test site_interaction_schema`
exits 101 with **one pass and three expected failures**: old v3 omission passes;
sparse Interaction, disabled-section validation and complete plan examples fail
because `SiteRule` rejects the missing `interaction` field. Evidence:
`schema-contract-red.log`. These are new, deliberately failing P00 scaffolds,
not pre-existing baseline defects. No test is ignored. The next schema work must
make them pass without default-filling sparse serialization. Examples copied
exactly from Sections 8.1–8.3 (including the root fragment in its collection)
live under config `tests/fixtures/site-policy/`. Interim review verified sparse
oracle construction and disabled-section precheck.

SDK receipt captured with existing schema-v2 tool:
`python3 tools/qualification/kde_free.py sdk --prefix
/home/tom/.local/share/ferric-browser-runtime/qt/6.11.2/gcc_64 --generated
target/site-policy-baseline/qml/ferric-protocols` → `sdk.json`, exit 0.
It hashes matching private header, Qt libraries, pinned protocol provenance and
all generated client/server/protocol files, with wayland-scanner 1.26.0.
This records baseline SDK inputs; it is not final installed/native qualification
and will require fresh source-bound receipts after implementation.

## Native prerequisite and independent P02 work

Production build completed successfully: `tools/performance/with_qt611.sh cargo
build --locked`, exit 0, 18m34s (`build.log`). Existing GCC16/Qt template warnings
are retained. The parser matrix process (82385) is still compiling its test
profile, which needs separate CXX-Qt artifacts for unwind-enabled tests.

Application harness configure failed, exit 1:
`tools/performance/with_qt611.sh cmake -S
crates/ferric-browser-engine-qt/tests/application -B
target/site-policy-baseline/application`. Exact blocker: qualified SDK lacks
`Qt6WaylandCompositorConfig.cmake`. Evidence: `application-configure.log`.
Attempt 1: inspected matching SDK component inventory; only WaylandClient,
ClientPrivate, GlobalPrivate and ScannerTools are installed. No retry with
system Qt6.12 or removal of portal/compositor tests is authorized by the plan.
Next native action: provision the matching 6.11.2 compositor SDK component,
then repeat configure with its explicit prefix. Goal remains active; independent
pure/config work is permitted by plan Section2 while this native gate is blocked.

P02 independent progress: `interaction_roots.rs` adds typed explicit root paths,
per-item paths, receiver scopes, exact browser-origin validation and depth/byte
bounds. It is **not** wired to SiteRule, discovery or native authority yet.
Reviewer found impossible self transitions and permissive legacy origin
canonicalization; both fixed and independently rechecked. Browser origins use
existing locked url2.5.8 directly (one Cargo.lock dependency edge; no new package
or version). Root paths start Document; per-item paths start Element; only slot
assignment returns Element. Constrained selector validation is a caller-supplied
stage and still needs P02's parser and engine/live integration.

Root/receiver final checkpoint: all 74 config library tests passed, exit0.
Independent receiver review and focused rerun passed; no remaining finding.
P02 and runtime integration remain incomplete.

Independent P02 predicates added in `interaction_predicates.rs`; 76 config
library tests pass. Reviewer identified an unintended ASCII-only restriction
on attributes/roles; fixed with bounded lowercase Unicode token validation,
HTML attribute separator checks and tests for data-é and Unicode roles.
Independent predicate rerun passed. Normalization, matching/proof and complete
SiteRule schema remain outstanding.

Native prerequisite remediation is underway in ignored target only. Official
QtWayland v6.11.2 source checkout resolves to
fd0456558bc1766da85aee42f6a87f79092cbb13. Configure with the qualified SDK passed:
`tools/performance/with_qt611.sh cmake -S
target/site-policy-baseline/qtwayland-source -B
target/site-policy-baseline/qtwayland-build
-DCMAKE_INSTALL_PREFIX=/home/tom/Projects/Ferric/ferric-browser/target/site-policy-baseline/qtwayland-sdk
-DQT_BUILD_EXAMPLES=OFF -DQT_BUILD_TESTS=OFF
-DFEATURE_wayland_server=ON -DFEATURE_wayland_client=OFF`.
The feature inputs were reported unused; actual cache enables both client/server
and compositor Quick integration. No claim of a compositor-only build is made.
Exact source/tag/configure logs retained alongside `qtwayland-build.log`.
Build: `cmake --build target/site-policy-baseline/qtwayland-build --parallel 2`.
Install and application configure retry await actual build completion.

Pending live handles at checkpoint: parser matrix 82385; QtWayland module
build 10650. Poll before starting replacement processes.

Independent P02 raw Interaction model added in `interaction_policy.rs`.
All planned patch fields are typed and strict; absence versus explicit lists
and empty tables survives serialization. Four plan examples round-trip at
model level; CSS row tolerance canonicalizes integer input to f64 without
changing value or field presence. Label-leaf, ID and explicit code helpers
cover their local grammar; full canonical keys, aggregate/effective validation,
selector AST and SiteRule wiring remain outstanding. No model is active policy.
`cargo test --locked -p ferric-browser-config --lib` passed 78 tests
(`config-model-tests.log`). Independent reviewer reran both model tests, passed.

Parser matrix completed successfully: session82385 exit0, both tests passed,
2,288 compatibility cells plus defaults/index/normalization boundaries,
`parser-matrix.log`. P00-03 executable source contract table is verified.
Full engine Rust library suite now running with qualified wrapper:
`tools/performance/with_qt611.sh cargo test --locked
-p ferric-browser-engine-qt --lib` → `engine-lib-tests.log`.

Current checkpoint: P02 selector AST added (`interaction_selector.rs`) with
1,024-byte bounds, explicit descendant/child relations, compound source,
type/ID/class and attribute presence/equality/prefix tests, CSS escapes and
quoted values. Unsupported combinators/pseudos/lists/namespaces/comments/
universal selectors reject; existing general hints.selectors are unchanged.
Independent reviewer reran both original grammar tests and found no defect.
Added replacement-codepoint/CRLF escape cases and exhaustive two-character
ASCII adversarial input checks; all 82 config library tests pass. Engine CSS
validation, all-source selector enumeration, runtime boundary enforcement and
proof work accounting remain incomplete.

QtWayland initial build failed at VulkanServerBufferIntegrationPlugin because
Vulkan headers are absent (PFN_vkAllocateCommandBuffers and related types).
Attempt1 corrective action: provision public Vulkan-Headers v1.4.363 matching
installed loader 1.4.363.0 in ignored target, pinned to
6802bb4733b63ed5efd3adb308a6c885ef180ea1. Reconfigure with Vulkan_INCLUDE_DIR
and CMAKE_CXX_FLAGS pointing to its include directory passed. Build retry uses
qualified wrapper and retains `qtwayland-build-headers.log` (live session78050).
No upstream source or feature requirement was changed. Original failure log
is preserved. Full engine Rust tests remain running in session46295.

Authored validation stage added in `interaction_validation.rs`, including
selectors of disabled policies and all root/per-item/slot positions, exact
selector and serialized-byte boundary tests, IDs/families/list bounds,
container/primary alternatives, identity/attribute/tolerance checks and raw
source/context duplicates. Canonical-key and effective codebook validation
are explicitly separate, so this stage cannot authorize publication. Reviewer
independently ran four new tests successfully. Extracted collection validation
helper, applied Clippy fixes and reran: 86 config lib tests passed; config
all-target Clippy with -D warnings passed (`config-clippy.log`).

Engine library suite completed: 572 passed, zero failures; one pre-existing
opt-in `s17_rust_hint_pipeline_performance` ignored until explicitly measured.
Evidence `engine-lib-tests.log`, exit0. QtWayland corrected build completed,
then local install passed (`qtwayland-install.log`). All 575 install-manifest
files are under the project target prefix. Application configure with
FERRIC_QTWAYLAND_PREFIX pointing at that prefix finds WaylandCompositor;
next error requests exact FERRIC_ENGINE_CXX_INCLUDE_DIR metadata. Existing
debug archive depfile identifies its originating engine out directory; release
application qualification is still outstanding. No qualification is claimed
from merely configuring dependencies.

With compilation quiescent, started the existing hint performance QML suite:
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software
QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu tools/performance/with_qt611.sh
 target/site-policy-baseline/qml/qml_tests -input
 crates/ferric-browser-engine-qt/tests/qml-perf/tst_hint_performance.qml.
Raw output `hint-performance.log`; live session53117. Do not compile or run
competing benchmarks while this measurement is active. Profiled samples are
reported separately from unprofiled timing by the existing harness.

Prepared single typed LogicalKeyToken in core/key_token.rs, replacing legacy
Mixed allowlist parser rather than retaining two source grammars. Canonical
modifier/case rules, Enter/Return, PgUp/PageUp, Plus, Space and Backtab aliases
are represented as typed key+modifiers. Mixed reserved-key checks now follow
canonicalization, and equivalent aliases deduplicate. Legacy direct Unicode
single-grapheme/four-UTF16-unit bound remains. This is syntax, not native delivery
qualification; P09 must keep physical CapsLock normalization separate from
authored Ctrl+P alias interpretation.
Source-only reviewer found character reservation returned a base character for
modified keys; fixed to return a reservable character only when modifiers are
empty. Reviewer verified fix; no remaining source finding. New unit/Mixed alias
and rescue tests are prepared but **not run** while benchmark53117 is active.
Do not claim these changes compile or pass until that pending work executes.

Latest P02 integration checkpoint: SiteRule now carries optional Interaction
and all18constructors initialize absence. Static authored validation and key
compilation run through validate_site_rule, including disabled entries. Existing
v3 serialization remains unchanged without the section. Unsetting last ordinary
setting preserves Interaction. Explicit JSON null now rejects for all47new
optional positions (including SiteRule); omitted fields retain None. Source
reviews passed and independently reran schema/key/null tests.
Full validation passed181core +90config library +40IPC +6schema integration
tests. Core/config/IPC all-target Clippy -Dwarnings passed. Exact logs:
p02-schema-integration-tests.log, p02-schema-focused.log, p02-clippy.log.
Runtime hints/remaps are still unimplemented; this is schema/static foundation.

Baseline hint benchmark finished successfully:12test cases, zero failed/skipped,
605.241s. Raw log and unprofiled summaries retained, ten workloads100samples,
10warmups, nearest-rank p95/max. Representative p95collection500/100% =92.957ms;
5000/100% =1198.461ms; overlay500/100% =108.024ms. No p99claim; existing absolute
budgets still apply and are not relaxed by these observed values.

Application configure with matching component and original debug archive metadata
passed; build correctly rejected stale archive versus Cargo.lock. No source
freshness guard changed. Fresh release build launched using qualified wrapper,
--locked --offline --message-format=json (live99987), release-build.jsonl/log.
After completion extract Cargo's exact engine archive and generated header paths
before configuring/relinking application harness. P00/P02 phase gates remain open.

P02 authored-include checkpoint (8 November): independent review found that
include merging could remove invalid raw rules before typed validation. New
interaction_include_sources regression failed with "invalid included data was
hidden by replacement" (p02-include-red.log). Loader now validates each raw
source and retains typed, sparse authored site rules/Hint fields in an immutable
Arc catalogue beside effective Config. It does not reinstate replaced arrays.
Full config suite passed91library +5include-source +6schema tests. Follow-up
known-invalid/budget cases pending latest focused rerun; first source-only
review found no defect. Existing aggregate Hint selector validator is reused.
Config/core/IPC all-target Clippy passed after raw-string formatting fix.

Config-check enumeration now receives LoadedConfig intact, walks retained raw
sources, and uses catalogue-local document IDs instead of absolute paths in
renderer requests. New fixture preserves malformed legacy CSS hidden by parent
array replacement plus a disabled Interaction selector. Engine tests pending
96473, p02-selector-source-engine.log. Production QML surface tests passed6
cases, no skips (interaction-engine-css.log), with requested world forwarded.
Source-only review passed. Startup/reload/publication catalogue propagation is
still outstanding; do not claim full all-source validation or phase completion.

Latest source checkpoint supersedes pending items above: all6expanded include
regressions passed, including known-invalid disabled Interaction fields and
exact/over32KiB raw shared selector budgets. Config/core/IPC/application
all-target Clippy -Dwarnings passed (p02-source-clippy.log). All164application
library tests passed (p02-source-publication.log), including actual profile
fallback installation and atomic source/config update rejection. Application
layers, reloads, runtime updates and profile requests now carry the source Arc.
Review caught/fixed stale active sources at profile completion and metadata loss
on authored-base fallback. Completed pending layers/fallback layers now retain
the exact pair; explicit synthetic built-in reset deliberately clears both.
Reviewer source-verified both fixes, no remaining finding. Initial5focused
engine tests passed; latest rerun57392 pending. Startup transport remains open.

Build99987 is terminal: source edits during the long native compile yielded
mixed-revision Rust dependency errors. Initial log/JSON retained under
release-build-initial.*. Current-source rebuild86664 launched, same qualified
command and output names. Native compilation outputs are reused; no freshness
check bypass or timestamp manipulation. Extract actual archive/header metadata
only after successful completion and recheck current source freshness.

Startup catalogue wiring checkpoint (8 November): new native shared QVariant
owns a Rust Box containing captured Config+ArcSources. This avoids JSON size
bounds that cannot represent every already-valid include graph (short symlinks
may resolve to unrelated long canonical paths). No global registry, file reread
for catalogue, QML source fields or new private Qt API. Source bundle must match
base and user input requires a valid bundle. Startup is sealed before notifying
Qt setters. Retained base input metadata supports retries; reload publishes the
new pair; popup handoff clones before notifications. Source review caught/fixed
a missed secondary call: ready primary now recaptures its current bundle and
secondary checks initialization failure. Reviewer verified fix, no remaining
source finding. Native ownership/destruction/Main QML/WebEngine tests prepared,
NOT RUN, so new transport is not qualified. Files startup_configuration.rs/h/cpp
and tests/application/tst_startup_configuration.cpp.

Adjacent verification passed272config/application cases (164+5+91+6+6), no
failure/skip, p02-startup-adjacent-tests.log. Format check and KDE-free schema-v2
policy passed (p02-startup-format.log, kde-policy-startup.log). Most recent5engine
selector tests passed before startup changes; new Rust/native tests remain open.

Build86664 is terminal: link failed with undefined new startup CXX bridge
symbols because the bridge changed after native compilation started. Preserved
release-build-pre-startup.jsonl/log. No weakening or timestamp bypass. Following
completed source review, source frozen for build61629 (live), same qualified
release command. Do not edit watched production inputs until it completes;
then select matching archive/include metadata, build startup native target,
run its tests and Rust ownership test, and proceed with remaining P00/P02 gates.

Startup native harness preflight passed: qualified Qt6.11.2 moc generated its
moc include under target, then c++ -std=c++17 -fPIC -fsyntax-only compiled
application/tst_startup_configuration.cpp against the exact current release
CXX-Qt generated include tree and qualified Qt pkg-config cflags. Exit0;
startup-native-syntax.log retains existing GCC16/Qt SFINAE warnings. This was
syntax-only: no archive linked, no freshness guard bypassed, no runtime claim.
Full native tests still require successful build61629 and matching archive.

Independent P00 progress while production inputs stay frozen for61629:
- Expanded SitePolicyBaseline passed8cases, zero failures/skips (4.011s),
  site-policy-fixture-mutations.log. Added tests assert same Element recycled
  with changed href invalidates its retained ordinary target before fresh
  collection, and delayed target insertion appears only in a later result.
  No enhanced witness, reservation or early-input routing claim.
- Full standalone native ownership suite passed96cases, zero failures/skips,
  19.897s, native-ownership-baseline.log/xml. Actual production C++ router with
  constructed offscreen Qt events, covering passthrough entry/repeats/release,
  Mixed-owner retention, native prompts, composition, recovery and lifecycle.
  Not physical input, new mappings or full Rust/GUI qualification.
- Inventory reviewer requested concrete selector/proxy consumer, constructor
  and serialization entry points; these were source-verified and added to
  site-policy-p00-contracts.md, with obsolete remaining-gap wording reconciled.
  Reviewer verified P00-05 complete; only that checklist item is now checked.
  P00 phase gate and later P03 implementation remain open.

Frozen production build61629 passed (11m50s). Cargo confirms build-finished
success plus exact static engine/browser artifacts; generated include path
comes from the matching build-script message. Retained metadata:
engine-build-metadata.json. Reconfigured existing application build with these
exact paths and local matching compositor SDK; startup native target linked
successfully and its unmodified archive freshness guard passed.

application_startup_configuration passed (1.50s):4QtTest cases including
setup/cleanup, zero failures/skips. Cases reject missing/wrong/mismatched/reused
startup handles; actual Main QML/WebEngine rejects a retained malformed CSS
selector after its included file changes to valid data, proving captured-source
validation. QML recapture also retains the pair for native secondary acceptance.
Evidence startup-native-test.log, startup-native-results.xml and full copied
startup-native-detail.log (1.429s inner suite). Existing offscreen Vulkan/Chromium
warnings retained. This is a source-transport/selector test, not native key or
physical-input qualification. Rust final-copy ownership test is compiling under
release test profile (7465), and broad application native harness is building
(37304). No watched production source edits until these finish.

Broad native application harness37304 built and retained the source/engine
freshness guard. Focused application CTest84808 passed all4groups, 398.75s,
120QtTest cases total, zero failures/skips: navigation usability23, Hint
controls51, DPR-2 controls6, action feedback40. Exact command:
FERRIC_QTWAYLAND_PREFIX=<local matching SDK> tools/performance/with_qt611.sh
ctest --test-dir target/site-policy-baseline/application -R
'^application_(hint_controls|hint_controls_dpr2|action_feedback|navigation_usability)$'
--output-on-failure --output-junit ../application-focused-results.xml
Evidence application-focused-tests.log, application-focused-detail.log and
application-focused-results.xml. Functional checks ran during compilation;
their durations are not performance qualification samples.

Inventory review found native matrix gaps in receiver policy scopes, exact
named-key/chord coverage and infrastructure-versus-capability classification.
Updated site-policy-native-matrix.json names all receiver policy branches,
95printable ASCII tokens plus each planned named key (including Escape), and
canonical parser-accepted modifier combinations requiring individual delivery
evidence. Added independent statuses and24explicit negative/lifecycle scenarios.
Reviewer verified P00-06 inventory complete; only this checklist item is now
checked. All qualification cells remain Not run and P00 exit gate remains open.

Rust startup lifetime test7465 passed after release-test-profile compilation
(20m02s). Its exact final-copy assertion confirms the Arc catalogue outlives
one QVariant copy and drops when the last owned copy drops. Full release engine
library suite then passed575tests, zero failures, one existing opt-in S17
performance test ignored (engine-current-tests.log, 0.66s). That opt-in test is
still required explicitly, with compilation/performance measurement separated.

Two legacy serialization tests now pass byte-for-byte against TOML generated
from original f2c5d6d source and unchanged baseline Cargo.lock. Isolated source
was created with git archive; only a dump example was added there. Baseline
configuration crate built --locked --offline (21.77s); outputs retained in
legacy-goldens, new regression fixtures under config tests/fixtures/site-policy/
legacy-v3. Reviewer verified provenance, optional-field/version/constructor
contracts and approved P02-01 individually. P02 phase gate remains open.

Workspace release all-target Clippy found two documentation-format errors in
the new startup module, fixed with identifier backticks. Original failure log
workspace-clippy-doc-failure.log retained. Rerun25077 is live. This comment-only
Rust change requires an archive refresh before another native relink; no
behavioral test expectation changed, no freshness guard weakened.

For future native builds, ccache4.14.2 was provisioned project-locally under
ignored build-tools from the official GitHub release. Official asset API digest
verified SHA256 d540ba01f160addbd3dd5e18eda9bd34005591b095c09ac8ae9d0a1b655e7ef5.
Receipt build-tools/ccache-receipt.json; upstream https://ccache.dev/download.
Uncached/miss/hit compilation of actual startup_configuration.cpp under the
qualified compiler/SDK produced identical object SHA256
0e0066c23b37500226b1d84c825026f84198d42c1c696ee7654e4118e4145b0b.
Probe timings5.257/5.843/0.042s are build-tool checks, not product performance.
Cache config/path are within target (2GiB cap, content compiler check); no global
compiler/shell/SDK configuration changed. For future commands explicitly set
CCACHE_CONFIGPATH and CCACHE_DIR to these project paths, CCACHE_REMOTE_STORAGE
empty, and prefix CC/CXX with this ccache binary. Do not change a running job's
environment or restart it to activate caching. Current Clippy continues unchanged.

Workspace all-target release Clippy rerun25077 passed -Dwarnings (7m46s),
workspace-current-clippy.log. Current format check passed, format-current.log.
After all compilation jobs ended, the required opt-in Rust pipeline test35188
passed (3.14s execution). Command: qualified wrapper cargo test --release
-p ferric-browser-engine-qt --lib --locked --offline
 tests::performance::s17_rust_hint_pipeline_performance -- --exact --ignored
--nocapture. Raw100samples plus10excluded warmups at500/1000/5000 candidates
retained in rust-hint-performance.log; nearest-rank p95/max and SHA in
rust-hint-performance-summary.json. p95 label assignment0.100/0.199/1.354ms;
decode2.050/3.487/15.455ms. No p99 claim or live DOM/native measurement.

performance-budgets.json now freezes per-stage ordinary p95 values and exact
Section11.2 planning ceilings before optimization: baseline+max(2ms,10%) for
no-policy stages, baseline+max(10ms,25%) for matched collection/fresh stages.
Existing stricter targets still win. DEVELOPMENT_SPEC.md explicitly sets the
500-visible-Hint end-to-end gate at200ms, and Normal key-to-visible at20ms p95 /
50ms p99. Per-stage percentiles cannot be summed to prove end-to-end acceptance;
those measurements plus native input/cold/idle coverage remain open. This is
baseline/budget evidence only, not a later enhanced-feature performance pass.

## Native lifetime/reentrancy investigation (in progress)

The original router blob matched pinned baseline
`5097e2be46363648b8aea9c687eca5a63dfe29e3`. Isolated ASan regressions
reproduced destruction during key/clipboard admission and search focus/native
submission. Evidence is retained under `target/site-policy-baseline/` in
`native-lifetime-*-red.log`. The first fixes passed 13 QtTest cases and both
combined native CTest targets (`native-current-test.log`).

Independent review (`native-router-review.log`) found nested release admission,
clipboard ownership, release-before-press and search focus reauthorization
issues. Five added reproductions failed (`native-review-red.log`) and then
passed, producing 18 passing ASan cases. A second review
(`native-router-rereview.log`) rejected queued-release ordering and stale
ownership snapshots despite those passing tests. The current revision replaces
posting with guarded synchronous delivery, adds press generations and preserves
the pending admission chain across release callbacks. This revision is still
under review; it is not accepted native qualification.

A further test accidentally regenerated clipboard overrides without a recursion
bound. `native-review2-crash.log` records its ASan stack overflow in the test
callback. The callback now injects its nested press once. The hung Qt backtrace
collector and that test process were terminated explicitly before rerunning
with `-nocrashhandler`; no production process was stopped. Current verification
logs are `native-review3-build.log` and `native-review3-test.log`.

The production engine archive predates these router edits and must be rebuilt
before application validation. No phase gate is closed by this investigation;
physical compositor input and mapping qualification remain outstanding.

Follow-up: `native-review3-test.log` passed 21 ASan cases. Independent agent
`native_router_review3` found cross-window deferred release and downstream
filter ordering gaps. Fixes preserve the pending admission during synchronous
press delivery and require the original active receiver for deferred release;
physical key-up now prunes generation state. Added two regressions; all 23
ASan cases pass (`native-review4-test.log`). Reviewer confirms those fixes,
with pre-forward target-window authorization still outstanding. Active-window
pre-forward validation and an inactive-press assertion were then added;
combined native build/test session95771 is pending, with logs
`native-review5-build.log` / `native-review5-test.log`. Target retargeting must
also be checked before forwarding and in deferred-release cancellation before
this revision can be accepted. Production archive remains stale.

Final router review: independent agent `native_router_review3` verified the
pre-forward check now requires both the original active window and unchanged
router target; release cancellation applies the target check too. No remaining
confirmed source findings. Two separate test rows cover focus movement versus
retargeting without a focus change. Prior combined native run passed both
CTest targets (21.03s, `native-review5-test.log`). Final source-bound run is
session31370, `native-final-build.log` / `native-final-test.log`; verify its
terminal result before accepting validation. Production rebuild still pending.

Final native source-bound run31370 completed exit0: both CTest targets pass,
20.95s, with 96 native-insert and 24 ASan lifetime/reentrancy QtTest cases,
zero failures/skips. `native-final-results.xml` and LastTest.log retain details.
Production release rebuild is now running as session99223 with exact Qt6.11.2,
project-local matching QtWayland and validated project-local ccache; logs
`router-release-build.jsonl` / `router-release-build.log`. Keep production
sources frozen until this build completes, then derive matching artifact/include
metadata and run page-search, Hint-controls, action-feedback, clipboard and
startup application groups. This native checkpoint does not close P00/P01 or
prove physical/compositor mapped input.

While production rebuild99223 runs with production inputs frozen, added
`tests/fixtures/site-policy/frames.html`: depth-two same-origin frame plus a
sandboxed opaque-origin frame with explicit fixture-only message/focus
handshake. Added `test_frame_chain_and_foreign_origin_negative_boundary` to
SitePolicyBaseline. Real WebEngine discovers the deep same-origin anchor,
rejects ordinary access/discovery of the foreign anchor, observes focus in
that opaque frame, and records no activation. All9cases pass (5.478s, Qt6.11.2
QPA offscreen); `site-policy-frame-fixture.log`. This is a deterministic
foreign-origin subject and ordinary negative boundary, not production broker
implementation or native keyboard qualification. Independent fixture review
`p00_frame_review` pending. P00-08 stays open pending complete subject audit.

Fixture reviewer `p00_frame_review` found no defect in the opaque-origin
fixture, but correctly identified that it cannot substitute for the permitted
canonical-origin foreign-frame broker subject (P05 rejects opaque origins).
P00-08 also still needs delayed asynchronous collection completion coverage.
Existing `tests/frame-probes/run_probe.py` and frame-provider-parent/child
fixtures supply a reusable localhost/127.0.0.1 cross-origin HTTP harness.

Added a distinct side-by-side grid/sidebar fixture (`sidebar.html`) and
`test_distinct_grid_and_sidebar_collections`. It checks actual separate layout
and distinct ordinary target IDs for cards sharing the same href. Full
SitePolicyBaseline now passes10cases, zero failures/skips,5.889s,
`site-policy-sidebar-fixture.log`. Reviewer follow-up pending. Production
build99223 remains live and source inputs unchanged; P00-08 remains open.

Added canonical-origin HTTP subjects `canonical-parent.html` (127.0.0.1) and
`canonical-child.html` (localhost) plus `tests/site-policy-http/run_fixture.py`
and `tst_canonical_frame.qml`. Runner binds loopback8766 and fails if occupied,
serves only site-policy fixtures, bounds subprocess to90s and closes server in
finally. Invoked via exact SDK wrapper with existing qualified QML runner.
All3QtTest cases pass, zero failures/skips,722ms, retained
`canonical-frame-fixture.log`. Test verifies canonical child origin, parent
DOM denial, child editor and parent frame focus, then ordinary top discovery
exclusion while focused, child-scoped discovery and zero activation. It uses
public frame-scoped execution to establish a subject only; no production
broker authorization or routing is asserted. Independent review pending.
Sidebar reviewer found no remaining issue; its missing subject is covered.
Delayed asynchronous completion coverage and broader P00 gates remain open.

P00-08 fixture inventory completed after independent review. Reviewer confirmed
existing `hintEarlyInput(primary-pending-collection)` and secondary equivalent
cover delayed collection via real collection/no-session assertion, synchronous
early input, eventual exactly-once activation and empty queue; refresh rows
cover pending refresh. Verified all four PASS entries in retained
`application-focused-detail.log`. No synthetic delay replacement needed.
`site-policy-fixture-inventory.md` maps every required subject to a fixture and
executable test. Only P00-08 is checked; current source-bound application rerun,
performance/native qualification and broader P00 gate remain pending.

Remaining-P00 audit (`p00_remaining_audit`) identified broad existing ownership
coverage plus a missing production Ctrl+Shift+Escape regression. Added
`ApplicationTests::sitePolicyProtectedRecovery` with passthrough escape and
fullscreen-ending rows, CTest `application_site_policy_recovery`. Runtime is
NOT RUN pending rebuilt archive; no secondary recovery equivalence assumed.
Added `site-policy-no-policy-ownership.md` with exact existing mode/count/IME/
clipboard/prompt test map and evidence limits. Added
`site-policy-binding-contracts.md` indexing all twelve Section3 contracts,
module/interface owners, existing static/baseline evidence and later phases.
Independent source/doc review pending; P00-04/P00-07 stay open.
Production rebuild99223 remains live compiling qualified native bindings;
router-release-build logs retained. No production inputs modified during it.
After successful archive generation, configure matching application include
metadata and build application_tests/startup_configuration_tests; run recovery,
page_search, hint_controls, action_feedback, clipboard, startup configuration.

Remaining-P00 reviewer found two small corrections: Normal exact leaves
execute but ambiguous prefixes/counts retain ownership and may wait/timeout;
recovery must also assert protected Escape does not reach page keydown trace.
Both corrected. Recovery runtime remains pending; no unobserved key-up claim.
Mapped production prompts to existing promptAcceptCancelAndNestedRestoration,
productPolishPagePrompts and productPolishWindowPrompts, registered narrow
`application_site_policy_prompts` CTest for their rerun. Section3 index reviewed
adequate; linked from original contracts inventory and reconciled obsolete
parser/startup/fixture status prose. Source/doc follow-up review pending.

Independent follow-up review verified all source/doc fixes with no remaining
confirmed finding. P00-07 contract/interface scaffolding is complete and checked
in the plan; this does not claim future effective policy, codebook, broker,
ledger, recovery or UI behavior. P00-04 remains open pending the source-bound
application recovery/clipboard/prompt tests after production rebuild99223.

P00-12 mapping audit verified all112Section9 rows match plan IDs/scenarios/
expected/phases. Added explicit proposed file/harness ownership and assertion
scope for each row, with renderer payload/root cases owned by real renderer/
browser harnesses, platform input by native/compositor qualification, and
missing-infrastructure reporting by qualification tooling. Proposed locations
remain clearly future coverage. Attached existing parser-negative tests and
parser-matrix.log to T-H37 as partial baseline evidence. T-H31 unsupported
foreign transport negative assertions remain missing; P00-12 stays open.
Production rebuild99223 remains live; no source-bound application pass claimed.

P00-12 reviewer requested two remaining mapping corrections: T-P16 proof-decoder
pure tests belong in engine hint_payload.rs, T-H37 parser tests in engine
hint_policy.rs. Corrected proposed locations. Added actual production
`crossOriginUnsupportedScopedActions` pointer/scope rows: enter real foreign
frame via hint-inside-frame, retain genuine scopedOwner, call production
provider.collect, assert unsupported-frame-action and zero child activation/
pointerdown, exit and verify lease cleanup. CTest
application_site_policy_foreign_negatives registered. T-H31 records these
implemented-but-NOT-RUN cases; hostile region/surface browser-boundary negative
coverage remains pending. Separate callback result storage avoids overwriting
an asynchronous frame result with a synchronous evaluate result. Source review
and syntax-only test translation unit check13184 are pending; this does not
link or qualify the production archive. Rebuild99223 remains live.

Pointer/scope negative-test review found a false-positive before runtime:
MainWorld window counters were read through ApplicationWorld, so undefined
could convert to zero. Replaced them with DOM dataset counters shared across
worlds and required no broker error, both keys present and successful numeric
conversion before zero comparisons. Reviewer verified the fix, no further
confirmed source findings. Corrected test translation unit syntax check60610
passed (`application-negatives-syntax-fixed.log`); no archive linked and no
runtime pass claimed. Production build99223 has advanced from cxx-qt-lib to
ferric-browser-engine-qt, still live. P00-12/region/surface negatives remain open.

Added `crossOriginHostileFacetResults` region/surface rows to the production
application foreign-negative CTest group. They enter a real foreign frame,
hold ordinary UI collection, request the official family, collect with current
selector snapshot through genuine broker lease, production-project real
candidates, begin a browser session, provider-fresh-check the actual target,
and assert unsupported-frame-action/no action/no palette/valid zero activation.
No fabricated kind or frame handle. Tests are source-review pending and NOT RUN.
Syntax-only full application test translation unit passed75904, retained
`application-facets-syntax.log`; this links no archive. T-H31 manifest names
all four implemented-but-unrun negatives, enhanced integration still pending.
P00-12 remains open until negatives execute and review findings are resolved.
Production build99223 remains live in engine native compilation.

Facet-negative reviewer found no confirmed authority/boundary defect; requested
explicit kind assertions per row to prevent fixture drift weakening named
coverage. Added region/scrollable-region vs surface/focusable-surface checks.
Runtime must still verify held-collector and broker-lease setup assumptions.
Prepared ignored helper `target/site-policy-baseline/prepare_router_application.py`
to require Cargo terminal build-finished success and select actual engine
archive/build-script include metadata before application CMake configuration;
it preserves the existing archive freshness guard. Do not invoke until99223
finishes. Application tests, P00-04/P00-12 runtime evidence remain pending.

Production rebuild99223 completed exit0 after29m32s. Cargo terminal
build-finished reports success. Matching engine archive/include selected from
actual Cargo metadata in `router-engine-build-metadata.json` (local package ID
uses path#version, handled explicitly). Application CMake configured successfully
with matching QtWayland prefix and existing freshness guard. Harness rebuild
session18577 is live, logs `router-application-configure.log` and
`router-application-build.log`. Native sources remain frozen until the planned
source-bound recovery/foreign-negative/prompt/clipboard/Hint/search/startup
application tests complete. No application runtime success inferred from build.

Application harness build18577 passed. First new CTest run97375: recovery
passed4QtTest cases and prompts passed8cases; foreign pointer/scope passed,
but facet rows failed setup because registry rejects a new Hint command while
already in Hint mode. Retained full failed run logs/results; no product failure
inferred from invalid test setup. Changed explicitly hostile-result test to
set the existing browser-owned family expectation directly, preserving real
broker lease/collector identity/projection/fresh validation. Reviewer accepted
this decoder-state setup and its distinct boundary-test label.

Corrected rebuild/run55492 passed foreign-negative group13.07s,6QtTest cases
including setup/cleanup, zero failures/skips. Retained router-negative-tests.log,
router-negative-detail.log and router-negative-results.xml. T-H31 baseline
negative evidence is now partial-pass; enhanced integration remains open.
Recovery/prompts details: router-new-application-detail.log and matching XML
retain successful4/8case groups alongside original facet setup failure.
Source-bound broader application run27385 is live for page_search,
hint_controls, action_feedback, clipboard and startup_configuration. Logs
router-application-tests.log/results.xml. Keep production inputs frozen until
it completes. P00-04/P00-12 final evidence review remains pending.

P00-12 final mapping review verified112exact Section9 rows, concrete proposed
pure/integration ownership and scope, corrected T-P16/T-H37 locations and
passing four foreign-negative data cases. Reviewer requested T-H31 evidence
record correction, which current manifest already contains (verified from disk:
partial baseline pass and all3corrected standalone log/XML paths). Checked
P00-12 for scaffolding/baseline negatives only; all enhanced acceptance and
platform cells remain pending. Original router-new-application XML overall
failure is preserved; only its individual recovery/prompt successes are cited.
Current broader application run27385 remains live at page_search.

Source-bound broader application run27385 completed exit0: all5CTest targets
pass,425.61s. QtTest counts: page_search38, hint_controls51, action_feedback40,
clipboard81, startup_configuration4; total214, zero failures/skips. Retained
`router-application-tests.log`, `router-application-results.xml` and copied
`router-application-detail.log` before any further CTest overwrites. Together
with recovery4, prompt8 and corrected foreign-negative6 cases,232selected
application QtTest cases pass. These are offscreen production QML/Rust/router/
WebEngine integration, not compositor/physical input qualification. Native
router120cases and independent reviews remain separate evidence. P00-04 final
ownership/evidence review pending; native lifetime fix checkpoint now has
matching successful production build and relevant application regression run.

P00-04 final evidence review found no concrete remaining baseline gap and
approved closure. Plan checkbox updated for existing no-policy ownership and
fixtures only; physical platforms/enhanced routes remain open. Task-local
`with_cached_qt611.sh` retains exact completed build SDK/CC/CXX/cache settings
for future Cargo checks, with no global machine configuration change.

With compiler and application runs terminal, started performance baseline
session75051: existing check_hint_cold_path.py using qualified QML runner,
then existing HintViewportPerformance. Logs hint-cold-path-baseline.log and
hint-viewport-baseline.log. The differential cold harness exports its own
immutable historical reference30e9daf; preserve that meaning rather than
calling it the implementation-plan pinned revision. Current collector samples
are ordinary/no-policy baseline dimensions only. New enhanced-policy/native/
end-to-end measurements and full remaining P00 checks are still pending.

Performance run75051 completed exit0: HintCollectorCache16cases (142.401s),
HintViewportPerformance4cases (20.852s), zero failures/skips. Raw samples retained
in hint-cold-path-baseline.log/hint-viewport-baseline.log; nearest-rank summary
cold-viewport-performance-summary.json. Historical differential cold harness
uses20timing samples, so these remain diagnostic rather than prescribed100sample
gate evidence; no P99 or native/key-to-paint claim. Current collector diagnostic
p95:500targets62.20ms,1000targets121.30ms,5000targets2399.80ms. Viewport current
refresh stage (10warmups/100samples) p95:100markers10.476ms,500markers59.091ms,
synthetic geometry/production overlay only, excludes DOM/Rust/native paint.

Started remaining repository entry point using consistent cached qualified SDK:
CARGO_NET_OFFLINE=true with_cached_qt611.sh cargo run --release -p xtask --locked
--offline -- check. Log p00-xtask-check.log; process still running. It includes
actual downstream checks, not an inferred pass from existing focused results.
P00-10/P00-11 and phase gate remain open. Independent reviewer is assessing
minimal real input-to-visible and Hint end-to-end performance harness coverage.

Performance review found no existing end-to-end key-to-chrome harness. Added
opt-in `sitePolicyCommandRenderPerformance` test implementation, not yet built
or run. It sends constructed Qt colon down/up through production window/router,
checks actual command mode/surface/input/focus on GUI afterAnimating, captures
that generation at afterSynchronizing and timestamps matching frameSwapped via
shared atomic state. GUI waiter polling only wakes the loop; it does not define
the latency.10warmups/1000measured entries, reset frame outside timing. Endpoint
is offscreen Quick rendering, not compositor presentation. Raw sample output
prefix FERRIC_SITE_POLICY_RENDER_JSON; performance/source review pending. Do
not run measurements while repository check5022 is compiling/executing.

Key-to-render reviewer found no confirmed generation/thread/lifetime defect,
but timeout exits lost collected samples. Fixed with unconditional scoped raw
report including setup/warmup/measurement/reset stage, sample index, completed
samples and first-failure count. Compute nearest-rank p95/p99/max after1000
measured samples and enforce existing20/50ms ceilings. Separate run receipt
must supply matching archive/source identity, SDK/Chromium/machine and exact
command. Helper remains unbuilt/unrun; review follow-up pending. Repository
check5022 continues through ordinary QML suite; no perf timing while it runs.

Independent P02 static audit identifies remaining effective/provenance model,
compiled normalized name/evidence semantics, destination capability metadata,
effective limits/bounded diagnostics, dummy consumer boundary contract,
versioned bounded renderer proof types and property/fuzz coverage. It found no
additional confirmed static parsing defect; typed roots/receiver/keys/selector
work should receive targeted completion review. No P02 full gate claim.

Repository check5022 passed ordinary QML, profile-lifetime and download-source
stages, then began its actual workspace test build (default test profile).
Keep same live process; consistent cached SDK wrapper remains in effect.

Reviewed command-render helper application build35147 is live, logs
render-perf-configure.log/render-perf-build.log. Final evidence reporting fix
sets threshold-series failure index to-1. No performance measurement will be
accepted while compiler/check jobs run. Added local render-targets.html subject
with500/1000/5000anchors, fresh DOM per navigation, resize-aware visible geometry,
no collector warmup/event tracing/activation. It is not yet connected to the
cold Hint end-to-end helper, and remains unverified. P00-11 stays open.

First command-render application build35147 passed. Added cold Hint end-to-end
helper in same test translation unit:500targets at80/100/125%zoom and1000/5000
at100%,10warmups/100measured fresh documents each. Uses actual current session,
tracking view/collection epoch, real laid-out/displayed rows, instantiated visible
marker delegates and visible label Text, then matching synchronized/swapped
frame.500rows require500visible markers and p95<=200ms.1000/5000record actual
marker counts without claiming every candidate has a simultaneously separate
marker. Fresh-document readiness/initial realm and teardown outside measured
interval; shared30s per-Hint watchdog, partial JSON on assertion exits.
Syntax-only check24111 passed (`hint-render-syntax.log`); independent performance
review pending. No measurement yet: repository check5022 remains live compiling
workspace test profile. Future opt-in runs must record matching artifacts/source,
machine/SDK/input metadata and set sufficient QTEST_FUNCTION_TIMEOUT for all110
cold-document samples per row; no debug/compiler load accepted as idle timing.

Hint performance reviewer found actual zoom was not verified. Added per-document
actual WebEngineView.zoomFactor equality, validated before/after CSS viewport
sizes and recorded actual view geometry/zoom. Added completed sample count and
per-sample GUI predicate-time accounting. Reported latency remains inclusive;
observer overhead is recorded separately and never subtracted. Review follow-up
and updated helper compile/runtime remain pending; first command-only build
35147 is not evidence for these later Hint helper edits. No performance pass
claimed while repository test-profile compilation5022 runs.

Helper rebuild16520 passed. Functional smoke78084 (explicit smoke label, loaded
by concurrent repository test build) completed5passed/3failed. Command-render,
500targets100% and1000targets100% handshake cases passed;500targets80/125%failed
actual-zoom assertion (view stayed1.0).5000targets hit production frame-timeout
and the helper watchdog; retained partial raw/failure records in
render-helper-smoke.log. This is functional diagnostic evidence only, no idle
performance baseline or latency gate pass/fail inference.

Fixed zoom workload preparation to explicitly set actual native view zoom after
each fresh navigation outside timing, retaining equality assertion. Removed
reliance on setting acceptance. Benchmark observation/dispatch test source now
uses -O2 for GCC/Clang regardless of functional-harness build type; inclusive
generation observer cost remains recorded, never subtracted. Updated helper
build99620 is live (`render-helper-zoom-build.log`). Independent review pending.
5000frame-timeout needs idle reproduction after repository5022 completes; no
timer, target cap or threshold has been changed to conceal that result.

Zoom follow-up review accepted explicit setter and benchmark-only-O2, but requires
waiting outside timing for actual renderer CSS viewport=view dimensions/zoom
(with rounding tolerance) and fixture resize layout completion. Fixture now
records its applied layoutWidth/layoutHeight. Test readiness check still to add
after current build99620 terminates; do not measure with only positive viewport
sizes. Reviewer identified5000timeout source as production broker's fixed1000ms
operation watchdog (root collection uses it), not test30s deadline. Preserve
bound and rerun idle; no watchdog bypass/increase authorized by this result.

Zoom build99620 and renderer-readiness rebuild44246 passed. Readiness fix passed
source review. Loaded smoke13617 ended5passed/2failed (render-helper-zoom-smoke.log):
command and500targets100/125%passed,500targets80%failed concrete render endpoint,
1000targets hit production frame-timeout. These are smoke functional outcomes,
not baseline performance. No5000rerun under load; idle reproduction pending.
Added failure-only endpoint state snapshot (mode/status/current epoch/session/
tracking view, laid-out/displayed/repeater counts, actual zoom) to distinguish
500-at80% unmet endpoint without weakening500visible-marker requirement.
One-source diagnostic helper build85752 is live, log
render-helper-diagnostic-build.log. Repository check5022 remains live.

Diagnostic helper85752 passed.80%diagnostic7075failed with500session/laid-out
rows but498displayed markers, actualzoom.8/currentview/pendingfalse. Adjusted
500-target fixture25columns->20columns/25rows to provide3-character badge
spacing without changing target count/500-visible assertion or product limits.
Reviewer accepted workload fidelity, requires treating layouts as distinct
fixture revisions. Loaded sparse smoke97093:500-at80%warmup reached all500markers
but next collection failed underload;100/125%passed. Full results preserved.
No captured historical fixture hash existed at diagnostic time; future paired
baseline must capture finalized fixture hash before execution, not claim a
reconstructed hash was contemporaneously recorded.

Repository check5022 terminated exit1. Earlier stages passed, including1243QML
cases, profile-lifetime and download-source tests, release/license checks and
multiple Rust suites. Engine result574passed/1failed/1ignored: obsolete private
field spelling assertion pageOwnedCurrentEvent_ in presentation_boundaries.rs.
Removed that assertion only; public interface checks remain and actual nested
ownership is tested by native-input-lifetime. Independent reviewer approved.
Full engine --lib correction run26949 is live; ccache records141additional
preprocessed/direct hits rather than another cold native build. After it,
rerun final cargo test --workspace --locked because5022 stopped before later
workspace suites. Earlier expensive stages need no repetition unless claiming
new aggregate xtask success. Keep5022 failed. Refresh matching production
artifacts after this src/test edit before valid application/performance use;
archive freshness guard remains binding. Idle broker-timeout reproduction and
full performance runs remain pending.

Corrected engine run26949 passed575tests, zero failures/1ignored. Final workspace
rerun54750 passed1350tests, zero failures/2ignored across21suite results, including
later suites skipped by originalfailed5022. Keep original aggregate failure;
its earlier1243QML/profile/download/source-stage passes plus corrected final
workspace command are separate evidence, not a new aggregate xtask success.

Cargo production refresh command completed successfully but cached the
cfg(test)-only source edit; existing archive preflight rejected stale src/test
mtime (`router-refresh-preflight.log`). Performed explicit package-scoped
cargo clean --release -p ferric-browser-engine-qt and release --lib rebuild,
process76470 remains live, logs router-engine-refresh-clean.log and
router-release-refresh-rebuilt.jsonl/log. No archive timestamp/guard bypass.

Added run_site_policy_performance.py reproducibility runner: exact source input
hashes (including CMake settings), binary/archive SHA, machine/buildflags and
explicit smoke/full environment; archive guard plus binary-input freshness;
bounded owned process-group execution, partial artifact finalization and parse
error preservation. CPP now emits runtime Qt/WebEngine/Chromium fingerprint and
per-sample events outside timing. Review found/fixed interruption/extraction and
orphan escalation bugs. Real lifecycle probe verifies a SIGTERM-ignoring child
is retired even if its leader exits; performance-runner-lifecycle-probe.log.
Runner follow-up review/build/runtime pending; this metadata is not a release
qualification receipt. Full idle performance samples and P00-10 remaining full
suite/platform command audit remain open.

Production package rebuild76470 completed exit0 (4m20s). Started browser refresh
61135, which selected another engine build context. Harness configuration90280
raced its generated headers and failed at the existing include-directory check
before linking. Preserve render-final-configure.log as failure; no application
binary linked from mixed/incomplete inputs. Let61135finish, then select completed
Cargo metadata and configure/relink sequentially. No measurements during builds.

Runner review found/fixed final status mismatch: child exit is retained separately;
no/missing/invalid result rows, parse errors and timeout now set final runner and
metadata status consistently before save. Expected smoke/full sample counts and
result-row counts are validated. A real executable-output negative probe passed:
valid sample survives truncated final record, child0 normalizes to runner/metadata1,
no performance result claimed (performance-runner-extraction-probe.log and
runner-extraction-probe/ artifacts). Earlier process-group lifecycle probe also
passes. Final runner review pending; full baseline and P00-10 workflows remain open.

Browser refresh61135 completed exit0 (4m30s); sequential combined Cargo metadata
selection/configure/application+startup build63283 passed, no mixed-header link.
Arch metadata/desktop validation passed via optimized xtask package arch
(p00-package-arch.log), not an installed-package runtime qualification.
Runner final review reports no remaining confirmed finding; parse/extraction and
process lifecycle probes retained separately from real performance evidence.

Full idle command-render run67300 passed with1000measured samples/10warmups,
no parse/result validation errors and matching child/runner0. p95=10.355198ms,
p99=14.993810ms,max=18.307834ms, within existing20/50ms ceilings. Artifacts under
command-render-baseline/: exact source/binary/archive/machine/build environment,
Qt/WebEngine6.11.2,Chromium140.0.7339.225/security151.0.7922.71, raw1000samples
and per-sample events. Pre-run one-second CPU probe92.29%idle; no compiler jobs.
This qualifies only constructed colon entry to production Quick-render endpoint,
not all Normal commands, compositor presentation or screen scanout. Frozen p95
no-policy planning ceiling12.355198ms in performance-budgets.json; existing
absolute ceilings remain binding. Full Hint baseline52680 is now live, artifact
directory hint-render-baseline; no other compiler/test workload running.

Full idle cold-Hint run52680 completed with child/runner4,5result rows,441sample
events and no parse errors.100measured samples each:500at80%p95581.783624ms,
500at100%577.433311ms,500at125%615.860342ms—all fail existing200ms ceiling.
All completed500-target samples satisfy500visible delegates.1000targets at100%
completed100samples,p95901.831824ms.5000targets failed in first warmup with
production frame-timeout,0valid latency samples, no overlay/session; record
unavailability rather than inventing a percentile. Exact source/binary/archive/
fixture/machine/runtime evidence, partial failure state and allrawsamples are
under hint-render-baseline/. No watchdog/cap/threshold change. Budgets record
current failures and keep binding200ms ceiling. These current ordinary/no-policy
measurements do not establish failure origin relative to an independently run
pinned executable; do not mislabel unproven provenance as pre-existing.

Started complete optimized xtask test application workflow15970 (p00-full-
application.log), including CTest inventory then CLI IPC/window IPC probes.
Current stage builds all application/backend/compositor targets; focused232cases
remain distinct evidence. Performance review of exact remaining P00-11 gaps is
pending. P00-10/P00-11 and broader gate remain open; final performance remediation
and enhanced-policy/native/platform qualification are mandatory outstanding work.

Independent performance review recomputed raw percentiles and verified all
reported conclusions. Completed500-target failures are valid baseline evidence,
not missing samples; they require remediation without threshold changes.
1000-target100samples distinguish63displayed cluster representatives from1000
candidates.5000availability failure has no usable-overlay percentile.
Remaining P00-11 ordinary-path dimensions: ready Hint selection/query and queued
input-to-render timing; actual application passive refresh;1000relevant mutations
(the existing1000-mutation scenario is1000targets/100individual mutations);
root-heavy timing subjects and no-Hint idle/resource baseline. Existing Rust
allocation/renderer/fresh/overlay stages need no redundant rerun. New matched
policy/remap/full-effective performance waits for owning implementation phases.
P00-11 stays open; final performance qualification remains failed/incomplete.

Prepared a separate render-refresh-targets.html fixture for the missing actual
passive-refresh/1000-mutation workload. Each explicit mutate() changes exactly
1000target style attributes via alternating1px transforms, retains identity/href/
names, and returns actual write counts/revision. It has not yet been wired to
or run through a measured production passive-refresh test; no coverage claim.
Frozen cold-render-targets fixture remains unchanged for paired comparisons.
Full application workflow15970 continues building its complete native inventory.

Refresh fixture functional run3544 passed3QtTest cases, zero failures/skips,
3.967s (`refresh-fixture-functional.log`). A real test-owned MutationObserver
asserts exactly1000style-attribute records per call, same DOM nodes/hrefs/IDs,
and production recollection sees1px geometry change then restoration. Reviewer
found no defect. This proves workload/recollection fidelity only; actual
production dirty-observer-to-refresh-to-render timing remains unimplemented.
Repeated sameNodes query checks are untimed diagnostics and must not be copied
into a measured interval.

Complete application workflow15970 remains live. Early live-Wayland seat-probe
cases failed initTestCase activation (including an initial-focus-requested
wrapper case); other offscreen groups continue. No platform support inferred.
Read-only native review requested without launching/focusing competing surfaces;
classification/provisioning outcome pending. Preserve p00-full-application.log
and its eventual aggregate status; focused offscreen232cases remain separate.

Added opt-in application `sitePolicyPassiveRefreshPerformance` and runner
`--kind refresh`: one continuous Hint invocation, 1000 actual style mutations
per sample, production dirty observer/80ms poll/collection, then stable labels,
all1000 candidate identities/geometries and real marker delegates at the
matching synchronized/swapped frame. Full mode requests10warmups/100samples;
functional smoke requests1warmup/2samples and makes no latency claim. Initial
review caught the incorrect assumption that browser HintSession IDs survive
refresh: production preserves HintInteraction but publishes a fresh session
authority per collection. Endpoint now requires a new nonempty ID each sample,
stable navigation target/selector snapshot, newer epoch and mutation receipt.
Build38923 passed. First functional smoke87160 failed at the initial session-ID
assertion because the test read a workspace property from the host window;
fixed the test to read the real Hint workspace. Incremental build42992 passed.
Second smoke37916 established the1000-target session with63displayed cluster
representatives, dispatched1000real mutations (revision1,total1000), then
production collection reported frame-timeout and closed Hint mode. Test2pass/
1fail,32.916s; child/runner1, one retained failed warmup event, no parse errors,
zero measured samples. Artifacts: passive-refresh-smoke/ and
passive-refresh-smoke-2/ (exact source/archive/binary fingerprints and command
in run-metadata.json), passive-refresh-build.log and passive-refresh-build-2.log.
Both runs were functional smoke while full application CTest was active;
neither is an idle latency baseline. Cause relative to concurrent load remains
unproven. Do not retry this gate until that workload ends; retain the fixed
1000mutations and production watchdog. Review of corrected session semantics
found no further confirmed source defect. Idle smoke/full timing remain
pending. No P00 checkbox closed. The still-running full application binary's
785 source inputs are preserved in full-application-source-snapshot.tar.gz and
full-application-source-inputs.json before subsequent test-only harness edits.

Full application workflow15970 is terminal: exit1 (CTest exit8),99/106groups
passed in7419.97s. Six host Wayland groups fail activation/owned-focus
preconditions; foreign-negatives fails region fresh callback count0instead1.
The offscreen aggregate and nested-Wayland groups passed. Retain aggregate
failure and frozen source/binary snapshot; skipped IPC probes are separate
follow-ups. Owned seat diagnostic exit1,42passive samples/zeroactive, no
wl_keyboard.enter; loaded client/plugin are qualified SDK6.11.2. Compositor
reason remains unknown. Current pgrep finds no hyprlock/hypridle; no OS changes.

P02-08 static capability requirements reviewed complete. Full release config
suite128pass, release all-target Clippy -Dwarnings pass, sixfocused tests pass.
All95ASCII and planned named destinations represented;128exact requirements
max, canonical overlapping/disjoint contexts and failed-apply retention covered.
Qualification remains required, never granted by syntax; full origin admission
stays separate. Evidence p02-capability-{tests-final,config-suite,clippy-final}.log
and source hashes. No new native support or complete P02 phase claim.

Separate original-browser follow-ups passed: cached Qt wrapper + python3
crates/ferric-browser-engine-qt/tests/application/run_ipc_probe.py --executable
target/qt611/release/ferric-browser (p00-frozen-cli-ipc.log), and
run_window_ipc_probe.py with the same executable and --record
target/site-policy-baseline/p00-frozen-window-ipc.json (corresponding .log).
Both exit0; window probe records13checks. Binary fingerprints are retained in
p00-frozen-followup-binaries.sha256. These fill skipped baseline IPC evidence
without changing the failed106-group aggregate. Independent Hyprland source
review of exact installed0.56.2commit confirms the Lua window selector is
correct; focus dispatch success is an acknowledged request, not focus proof.
No additional host focus retries justified until refusal/redirect cause is
identified. Required host/native gates remain open.

Foreign-negative controlled red reproduced the region failure on current
release engine/harness (foreign-race-engine-build.{jsonl,log}, configure/build
logs, foreign-race-red-test.log/results.xml, red source and hashes). Real All
collection in flight; naive pending=true hold; fresh callback0; final status
selector policy changed during collection, mode normal, collection6to8/provider
0to3, owner cleared. Corrected test waits that real All collection to finish,
atomically acquires pending gate with a C++ latch (Qt QTRY re-evaluates), asserts
Hint/All/unchanged scoped owner, then injects family expectations as before.
No production change or weakened freshness/epoch/unsupported-action assertion.
Reviewer found no remaining issue. Current foreign-negatives CTest passes
13.45s,6QtTest cases including setup/cleanup (foreign-race-green-test.log,
foreign-race-green-detail.log, green hashes). Original106-group failure remains.
An initial red invocation used unsupported QtTest output format junit; it ran
no tests, is preserved as foreign-race-red-invocation-error.log, corrected to
junitxml before the substantive red.

Additional P02-05 static boundary integration tests pass3cases (p02-predicate-
boundaries.log): decoded255/256/257UTF-8bytes including JSON escapes, allfour
finite/inclusive range domains and exact href origin/path grammar. No invented
256byte href-path limit: authored/effective serialized caps still apply. Full
compiled attribute/role/href/geometry evidence and shared-work matching remain
implementation work; P02-05 stays unchecked.

Idle passive-refresh smoke86956 (current rebuilt source/engine/harness)
terminated runner/child1; firstwarmup1000actual style writes, production broker
frame-timeout, Hint closed,0valid latency samples,33.596s. No concurrent tests
or builds. Artifacts passive-refresh-idle-smoke/ include exact hashes, command,
raw endpoint and mutation receipt. This establishes failure under idle test
conditions, not origin relative to pinned baseline. Stop further gate retries:
attempt1fixed workspace-ID read, attempt2under-load timeout, attempt3idle
timeout. Next eligible work is opt-in per-broker-operation timing/provenance
and profiling before an evidenced optimization, with unchanged1000mutations
and1000ms broker deadline. Source review rules out expensive per-mutation
observer callbacks but does not identify which broker operation times out.
63cluster markers do not qualify required500visible-marker workload. P00-11
and all final performance gates remain open. Independent P02 work continues.

P02-05 compiler complete for static scope after two independent reviews.
Finalrelease config141pass including10compiler tests and3decoded-value/range
boundary tests; adjacent application164pass; release all-target Clippy
-Dwarnings, format/diff/KDE-free policy pass. Logs p02-predicate-config-final,
p02-predicate-application-tests, p02-predicate-clippy-final,
p02-predicate-kde-policy; source hashes p02-predicate-source.sha256.
Primitive/schema/static tests do not qualify actual renderer completeness,
root identity, projected geometry or authority. Shared glob correction is
explicitly recorded in plan13.4 and no-policy index; actual red twofailures
(p02-predicate-glob-red.log), independent116281pair oracle and public existing
site/context regressions green. First focused run exposed incorrect expected
4worksteps instead5; later valid matcher reached a bad test fixture using
unsupported site hints.min_chars (dev-green.log), fixed to content.zoom.
Initial dev Clippy found one110line test, split rather than suppressed.
Finalsource all included in release suite. Full phaseP02gate stays open;
new runtime sources invalidate archive freshness for later native/perf runs.

Partial P02-13 fuzz work reviewed: interaction_policy target/nine corpus seeds;
strict roundtrips and shared-budget predicate evaluation; independent policy/fact
mutation, unavailable-facts invariant and literal attribute oracle. Three final
release corpus tests pass (p02-fuzz-seed-tests-final.log). Final aligned-lock
xtask corpus smoke passes13targets/32runs each (p02-fuzz-workflow-final.log),
strict target Clippy and release config+xtask all-target Clippy pass; both format
checks and diff check pass. No coverage instrumentation: do not call this a
coverage-guided campaign. Source hashes p02-fuzz-source.sha256. Explicit fuzz
targetdir fixes wrapper CARGO_TARGET_DIR mismatch. Fuzz lock95packages uses92
exact production identities plus3unchanged fuzz-only identities. Initial
ICU2.2build excluded; aligned build uses production2.1.1. Initial target Clippy
nested-if fixed with let chain; first logs retained. P02-13/phasegate stayopen
until proof-envelope and publication atomicity coverage exists.

Next eligible P02-12: strict versioned annotation-only envelope decoder beside
existing hint decoder; exact browser-captured rule/root/candidate/revision
associations; no renderer-authored action/command/transport. Keep ordinary
candidate payload separate. No existing aggregate result cap found; proposed
2MiB annotation budget is a documented implementation choice (plan13.4),
not a new whole-hint or config limit. Bounds/completeness/live transport tests
remain implementation work; no P02-12 checkbox closed.

P02-12 first pure engine annotation decoder implemented, not integrated with
live transport. First8release unit tests pass (p02-proof-first.log), including
separate5000×8192byte-href ordinary payload through existing decoder and valid
annotation. External enum tags preserve streaming sequence bounds. Snapshot
p02-proof-first-source.tar.gz. Review found parent-host/kind, cross-root mapping,
focused ancestry shared-depth gaps and an invalid name/display equivalence
restriction. Ten targeted regression tests currently running against unchanged
firstvalidator (p02-proof-regressions-red.log); source archive retained. No
P02-12 completion claim. Corrective implementation and broader boundary tests
remain next work; no GUI/native/platform/performance gate changed.
