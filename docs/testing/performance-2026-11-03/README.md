# Performance investigation, 3–4 November 2026

Baseline: `6d02cfe642cdbdb21dd798d897643cafcaaee018`. Work is isolated on
`perf/measured-efficiency`; the original checkout remains unchanged. Remote
verification and final validation receipts are recorded in the accompanying JSON.
The investigation began on 3 November; final qualification continued on 4 November.

All nine findings have a recorded disposition. Seven scoped optimizations are
retained, with the measurements, regression checks and limitations below.

## Findings and decisions

| ID | Classification | Observed cost and retained decision | Risk and limits |
| --- | --- | --- | --- |
| [PERF-01](collector-baseline.json) | Confirmed bottleneck | Current release DOM collection at 5,000 candidates: median 1,661 ms, p95 1,739 ms. Visibility and local hit testing dominate. No production collector rewrite retained. | Existing per-pass style/geometry/ancestor/frame caches already remove duplication. More selective mutation handling requires evidence on arbitrary CSS/attributes; cross-pass target caches would add invalidation risk. Current activation preflight remains intact. |
| [PERF-02](viewport-comparison.json) | Resolved | Reuse an identical viewport snapshot instead of notifying the overlay twice during a hint refresh. At 500 synthetic markers, layout batch median 140.96 → 70.11 ms. | Production overlay on Intel OpenGL, synthetic geometry. No target/activation cache; unchanged marker geometry checked. |
| [PERF-03](matcher-comparison.json) | Resolved | Stream the bounded fuzzy subsequence scan without temporary character vectors. At 5,000 candidates, representative median 7.66 → 2.98 ms. | Unicode, scoring, bounds and deterministic ranking checked against the previous algorithm. Decoder unchanged: current release p95 23.00 ms at 5,000, much smaller than historical debug timing. |
| [PERF-04](worker-paired-20.json) | Resolved | Defer seven per-owner helper workers until first use. Twenty page windows: median 673 → 532 process-tree threads in five paired runs. | Same per-profile ownership, channels, ordering and drop behavior. Memory and CPU differences are inconclusive; no claim of memory saving. |
| [PERF-05](evidence-batching.json) | Resolved | One aggregate evidence notification per batch, direct site-count access and Qt conversion outside the shared lock. 300 → 100 expensive refresh triggers across 100 synthetic batches. | Legacy property signals preserved. Blocking enforcement remains synchronous and unchanged. This is removed diagnostic work, not a measured network-latency gain. |
| [PERF-06](storage-comparison.json) | Resolved | Real idle timer calls fell from 100 to 5 per five seconds (-95%). Admission and index-completion wakes retain prompt delivery. | Same owning UI processes all completions; pending work polls at 50 ms, idle fallback is one second. Three after processes complete all 300 measured refreshes within 13.14 ms. No energy claim. |
| [PERF-07](projection-comparison.json) | Resolved | Build JSON only for the selected result page. Public uncached query median 7.75 → 2.55 ms at 1,000 retained rows. | Five native process pairs pass gain/regression gates. Privacy checks, ranking/ties, validation, wire fields and pagination preserved. |
| [PERF-08](tab-comparison.json) | Resolved | Replace repeated linear membership tests during tab-view retirement with a per-pass null-prototype lookup. At 500 aligned tabs, median 4.87 → 3.10 ms. | Actual controller/ListModel with Item views; excludes WebEngine rendering. General fallback reconciliation remains unchanged. |
| [PERF-09](hostname.json) | Not reproduced | Native release hostname misses: eight wildcard rules median 1.56 μs; 64 rules 9.08 μs; maximum 1,024 wildcard rules 123.62 μs. No index added. | Synthetic rule sets; no evidence that common lists justify extra policy/index complexity. Existing precedence and attribution unchanged. |

## Source locations

All paths below are relative to the repository root. The raw comparison files
identify individual source hashes and independently frozen binary hashes.

| ID | Implementation inspected or changed |
| --- | --- |
| PERF-01 | `crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js`, `qml/components/FerricHint{Workspace,FrameProvider}.qml` in that crate |
| PERF-02 | Engine `qml/components/FerricHint{Workspace,Overlay}.qml`, `qml/scripts/HintPresentation.js` |
| PERF-03 | `crates/ferric-browser-core/src/{switcher,hints}.rs`, engine `src/hint_payload.rs` |
| PERF-04 | `crates/ferric-browser-runtime/src/worker.rs`, engine `src/browser_ui_profile_setup.rs` and owned domain worker wrappers |
| PERF-05 | Engine `src/request_interceptor.{h,cpp}`, `src/blocking_evidence.rs`, primary/window-lifecycle/page-view QML handlers |
| PERF-06 | `crates/ferric-browser-application/src/{lib,storage}.rs`, engine `src/browser_ui_{library,maintenance,profile_setup}.rs`, `qml/components/FerricStoragePollScheduler.qml` and primary/secondary windows |
| PERF-07 | Engine `src/browser_ui_query_switcher.rs`, `src/switcher_presentation.rs` |
| PERF-08 | Engine `qml/components/FerricTabViewController.qml`, `FerricRuntimeController.qml` |
| PERF-09 | Engine `src/request_interceptor.cpp` (`hostMatches` and policy caps) |

## Corrections to the original handover

S25's approximately 99 ms decoding and 71 ms ranking p95 values were measured
from `target/debug/deps`, according to its manifest. They are historical debug
measurements, not current release baselines. The collector already caches
styles, ancestor visibility, rectangles, frame mappings and rendered state
within a collection. Existing spatial bucketing and representative hint markers
remain useful and are preserved.

The storage timer belongs to the primary window and visits durable window
entries. Twenty windows imply about 400 owner checks/second under the old timer,
not 400 independent QML timer wakeups. Idle blocked helper threads also do not
imply an equal number of running CPUs.

A complete qualified Qt/QtWebEngine 6.11.2 SDK was available locally. Initial
6.12-based qualification and the earlier SDK-blocker assessment are superseded
by the qualified runs in this report. The production SDK guard was not changed.

## Environment and measurement boundaries

Release builds use the repository's normal release profile and locked dependency
resolution. GPU tests run in an owned headless Weston session on Intel HD 620,
Mesa 26.2.5, Qt Quick OpenGL. Native application correctness and Switcher
projection fixtures use offscreen software rendering. The native Switcher
measurement excludes painting a visible frame. Exact machine, kernel,
distribution, governor and SDK facts are in the environment receipt.

This is a shared development machine under the powersave governor. Comparisons
use five alternating independent process pairs unless explicitly labelled as a
grouped matrix or structural count. Each latency run uses ten warmups and 100
samples. Confidence intervals bootstrap paired run deltas with seed zero;
individual samples from one process are not treated as independent runs.
No compilation runs concurrently with quantitative measurements.

A latency change is retained when median or p95 improves by both 10% and 1 ms,
with a paired confidence interval excluding zero, and no ordinary workload
regression above both 5% and 1 ms. Structural changes are assessed by the work
removed and their measured lifecycle cost, without inventing latency gains.

## DOM and hint presentation

| Inspected candidate workload | Release collection median | Release collection p95 |
| --- | ---: | ---: |
| 100 | 33.08 ms | 48.04 ms |
| 500 | 97.64 ms | 116.68 ms |
| 1,000 | 175.40 ms | 203.16 ms |
| 5,000 | 1,661.05 ms | 1,739.13 ms |

Raw callback timings are separate from instrumented profiling. At 5,000,
profiled medians include 1,128 ms local visibility, 761.45 ms local hit testing,
82.1 ms clipping and 130.8 ms description work. These stages overlap and must
not be added. A dynamic 1,000-node mutation workload was also exercised.
The original T043 overlay metadata hardcoded the word “software”; the runner
was configured for hardware OpenGL. Metadata now reports actual GraphicsInfo,
and the retained viewport comparison has GraphicsInfo=3 plus Intel GL logs.

A fresh current visible-overlay run with corrected metadata used native OpenGL
(GraphicsInfo=3, QSG_INFO device identity captured). At 500 markers and 100%
zoom, ten warmups plus 100 samples gave **126.872 ms median / 142.892 ms p95**
time-to-visible. This is a current rendering baseline, not a comparison with
historical software rendering or an attributed gain from the viewport change.

The viewport change removes an unnecessary assignment of an equivalent object.
The existing overlay consequently computes one layout rather than two for the
same refresh. Five process runs check identical layout output. The first
separate diagnostic run was excluded before comparison because it overlapped
smoke diagnostics; comparison runs 2–6 did not overlap builds or diagnostics.

| Synthetic refresh batch | Median before | Median after | Change | p95 before | p95 after |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100 markers | 22.696 ms | 11.090 ms | -51.1% | 27.046 ms | 16.740 ms |
| 500 markers | 140.962 ms | 70.106 ms | -50.3% | 155.505 ms | 79.170 ms |

Markers are hidden during this layout-only fixture. These are overlay layout
batches; they do not measure delegate creation, painting, time-to-visible or
end-to-end keyboard activation.
Ordinary and complex collector behavior, shadow/frame ownership and activation
preflight are covered separately by the QML/native regressions.

## Rust ranking and decoding

The matcher comparison compiles the actual old and retained `switcher.rs` with
`rustc -O`, without Qt. A sample ranks every candidate label. The first three
5,000-candidate workloads below pass the retention gate; the contiguous Unicode
control remains unchanged within uncertainty. None of the sixteen tested
100/500/1,000/5,000 workloads crosses the regression threshold.

| Query, 5,000 candidates | Median before | Median after | Change | p95 before | p95 after |
| --- | ---: | ---: | ---: | ---: | ---: |
| `synthetic target 42` | 7.663 ms | 2.984 ms | -61.1% | 11.305 ms | 4.696 ms |
| `st42` | 2.770 ms | 1.616 ms | -41.7% | 4.406 ms | 3.108 ms |
| `no-match-xyz` | 3.941 ms | 1.534 ms | -61.1% | 6.490 ms | 2.827 ms |
| `CAFÉ 東京` | 1.724 ms | 1.669 ms | inconclusive | 3.276 ms | 3.120 ms |

Current release decoder median/p95: 500 candidates 1.535/2.538 ms; 1,000
2.979/4.597 ms; 5,000 16.961/23.001 ms. Replacing its validation with a new typed
parser would have to preserve duplicate/null/href lenience and strict envelope,
size and identity contracts. That added complexity was not justified by the
current cost relative to DOM collection. No validations were removed.

## Worker resources

Seven helper categories keep their existing owner and dedicated channel/thread
once used. Configuration reload/write, printing, editor writes and profile
list/delete/preview start lazily. Eager worker construction remains available
for services that require it. Thread savings depend on which services remain
unused; first use starts and retains that owner’s existing dedicated worker.
Tests cover unused drop, failed first admission,
single-flight reuse, independent owners and nonblocking busy drop.

The paired twenty-window experiment uses the same frozen harness with two
separately frozen production browsers and thirty seconds of idle sampling.
The measured thread reduction is 141, paired 95% mean-delta interval
[-143, -139]. PSS 929,978 → 926,947 KiB has interval [-9,012, +6,640] KiB;
CPU and RSS intervals also cross zero. The earlier grouped PSS increase was not
reproduced as a statistically supported regression. Retention is based on the
thread reduction, without a PSS or CPU gain claim.

| Presentation | Pages | Threads before | Threads after (lazy workers) |
| --- | ---: | ---: | ---: |
| Internal tabs | 1 | 86.5 | 80.5 |
| Internal tabs | 5 | 122.5 | 113.5 |
| Internal tabs | 10 | 162.5 | 151.5 |
| Internal tabs | 20 | 239 | 233 |
| Page windows | 1 | 88.5 | 81 |
| Page windows | 5 | 210 | 177.5 |
| Page windows | 10 | 366 | 297 |
| Page windows | 20 | 675.5 | 537 |

The table is the exploratory two-run grouped matrix; the independent paired
check above is the stronger twenty-window evidence. Raw matrix files retain
PSS/RSS, main/tree CPU, startup/readiness, renderer/process/thread counts and
shutdown outcomes. No startup improvement is claimed.

A separate actual-worker integer-job benchmark shifts thread creation from
construction (20.73 → 0.40 μs) to first submit (0.14 → 19.81 μs). Total first
delivery is 43.57 → 40.35 μs; warm submit 1.37 → 1.42 μs. Domain-specific I/O is
excluded. This bounds the scheduling tradeoff rather than claiming that a real
print or profile operation finishes in microseconds.

## Storage and Switcher

The scheduler reuses the existing coalesced weak Qt-thread wake. Application
admission wakes the owning UI; asynchronous Switcher index completion also wakes
after releasing its result-slot lock. While application or adapter work remains,
the timer keeps the existing 50 ms bound. Idle owners use a one-second recovery
fallback. Pending checks include deferred start-page work, command history,
feedback, indexes, catalogs, sessions, flushes and recovery. Durable secondary
windows remain eligible even when the primary profile is private.

The real primary QML timer was observed using QSignalSpy. Five pre-storage
process runs from the retained pagination candidate each use ten warmups and
100 empty-history read/index refresh samples. Three isolated storage-only
process runs repeat the same fixture. This is a structural-count and completion
qualification, not a paired latency or power comparison.

| Metric | Before | After | Change / qualification |
| --- | ---: | ---: | --- |
| Idle polls in five seconds (median) | 100 | 5 | -95.0%; all three after runs have 5 |
| Idle timer interval | 50 ms | 1,000 ms | 50 ms retained while work is pending |
| Admitted refresh median | 50.017 ms | 5.117 ms | Grouped observation; no paired latency gain claimed |
| Slowest measured after refresh | — | 13.134 ms | All 300 after samples below the 500 ms fixture bound |

No changes were made to storage request execution, durability, profile ownership
or publication validation. The timer still calls the original complete polling
routine; it does not replace that routine with a storage-channel-only check.

The public Switcher comparison uses five alternating frozen before/after native
process pairs, ten warmups and 100 samples per query path. Each database is local
and disposable; the production library cap is **1,000 rows**, including when
the database contains 100,000. Both broad queries match every retained row.

| Database rows | Uncached median before | Uncached median after | Change | p95 before | p95 after |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 7.748 ms | 2.550 ms | -67.1% | 11.822 ms | 4.893 ms |
| 10,000 | 7.845 ms | 2.376 ms | -69.7% | 11.956 ms | 5.406 ms |
| 100,000 | 7.778 ms | 2.177 ms | -72.0% | 11.573 ms | 4.942 ms |

All three uncached workloads pass the retention gate and have negative paired
mean-delta confidence intervals. Cached medians remain about 0.6–0.8 ms, with
confidence intervals crossing zero; no cached-path improvement is claimed. No
ordinary-path regression crosses the gate. Typed rows retain the existing sort
keys until pagination; the existing JSON envelope and Qt list publication stay
unchanged. Exact wire metadata/null/Unicode and pagination tests pass.


## Tab reconciliation

The per-pass ID lookup removes repeated membership scans before retiring views.
For 500 aligned tabs, five independent process pairs give median 4.873 →
3.102 ms (-36.4%), p95 8.204 → 5.897 ms (-28.1%). Ordinary 20/100-tab controls
pass the regression gate. Reversed 500-tab fallback remains about 160 ms; a
larger reconciliation rewrite is not justified by the normal-path result.

## Harness validity and shutdown limitation

`xtask perf` schema 2 reports midpoint median, nearest-rank p95 and null p99
below 1,000 samples. `--samples` controls IPC samples per UI repetition; it does
not imply thirty launches or thirty CPU intervals. `runs` retain independent
repetitions; top-level measurements represent the first run.

Process-tree CPU matches `(PID, start-time ticks)` across endpoints. New child
CPU is included; exited children make the total unavailable because their final
counters cannot be reconstructed. Children born and exited entirely between
endpoints are not observable. CPU is relative to one logical processor and can
exceed 100%. PSS/RSS are endpoint process-tree sums; missing memory/thread
readers are explicit and incomplete thread totals are null. Wakeups, active
request-burst CPU and SQLite statement counts require external tracing and are
not inferred from these endpoint counters.

A fixture-only plugin confirms the native startup expansion prompt for more
than twelve requested windows and the rendered Quit control. It operates only
on an exact owned fixture profile and remains dormant during idle sampling.
It is not linked into the browser and does not change the production window
limit or durability policy. Qt Button.click is UI control qualification, not
physical keyboard/mouse input qualification.

The GPU multiwindow Quit workload fails to exit within thirty seconds on both
baseline and optimized builds. Captured state shows all participants prepared
and durable state flushed, with the final commit timer still running; the main
thread is in its normal event loop. The report preserves shutdown latency as
null with the error. Fixture cleanup kills are never reported as successful
shutdown. This pre-existing qualification failure requires a separate follow-up;
portable/native shutdown and persistence regression results are recorded below.

## Validation and implementation records

The exact command outcomes, source/binary hashes, test counts and qualification
limits are in [the final validation receipt](validation.json). Build and test
profiles are kept separate; repeated tests across release/debug/native runs are
not summed into a misleading unique-test total.

| Retained slice | Commit | Change, evidence and focused regressions |
| --- | --- | --- |
| Evidence batching | `6d66449` | Aggregate batch signal, direct site-count lookup, conversion outside lock; 300→100 refresh triggers. Native snapshot, clearing, reentrant delivery and QML count bindings. |
| Streaming matcher | `523c7d2` | Remove temporary character vectors; five-pair gain and no ordinary regression. Independent previous-algorithm oracle, Unicode, boundaries and 4,096-character limit. |
| Lazy owned workers | `428df36` | Defer seven native helpers until first submission; roughly 141 fewer threads at twenty windows. First-use cost and transport admission/reuse/failure/drop/owner tests. |
| Late Switcher values | `1f823e1` | Keep typed owned rows until pagination; five native pairs improve uncached queries 67–72%. Exact wire metadata/null/Unicode fields, ordering and privacy regressions. |
| Storage scheduling | `240565c` | Wake admitted work/index completion and slow idle polling; actual timer calls -95%, all 300 after refreshes within 13.134 ms. Pending-effect/owner scheduling and native completion/lifecycle regressions. |
| Viewport identity | `6f425d7` | Reuse equal viewport dimensions; two layouts become one, identical geometry, roughly 50% refresh-layout reduction. Viewport normalization and full hint/frame functional regressions. |
| Tab membership | `87e4794` | One per-pass membership map; 500-tab aligned reconciliation median -36.4%. Full reconciliation/presentation contracts and six repeated workload controls. |

Measurement/tooling commits include `78a219e`, `98d0d1d`, `737da06`, `147c1e7`,
`f564e0b`, `c202ce7`, `9851359` and `5dfc36e`. They add honest metric semantics,
separate collector instrumentation, native fixtures, source/ABI selection,
owned bounded UI confirmation and reproducible alternating comparisons.
Follow-up `001c38b` updates the source-contract regression assertions to verify
the extracted scheduler and both runtime wake paths. It changes tests only;
measured production code remains exactly the frozen `5dfc36e` product.
`316f442` repairs the existing config/blocking smoke fixtures: public CLI
initialization of owned roots, schema-v3 input files and short Wayland socket
paths. The incompatible-data guard is unchanged. `ecbb0fe` is a native fixture
correction that asserts the existing pending-management-action guard, then uses the
supported command path to change configuration before delivering a stale
selector callback. The previous fixture expected a second pending management
action to be accepted; production code already rejected it. No admission or
stale-callback guard was changed. None of these fixture changes alters the
production window limit or authorizes ordinary-profile automation.

Release workspace tests pass: **1,302 passed, zero failed, two opt-in tests
ignored**. The project gate also passes its debug Rust suite, **1,231 QML tests**,
two native profile-lifetime suites, the download-source suite, linting, generated
documentation, architecture, formatting and metadata checks. Full workspace
Clippy with all targets/features and `-D warnings`, release checking, the adapter
smoke, configuration reload smoke, real cached-list blocking smoke and native
evidence tests pass. Measurement tooling has independent Python, Node and xtask
regressions; exact commands and corrected attempts are retained in the receipt.

Twenty dedicated native application suites pass, including storage/session
recovery, durable shutdown, privacy, native ownership, hints/frames, downloads,
configuration and the public Switcher. Both probes using the actual release
browser (IPC and window IPC) pass. The monolithic offscreen attempt remains a
recorded failure/timeout, described below. Its remaining 128 application
functions were exercised in sixteen bounded batches: fifteen batches passed;
one had the obsolete selector-fixture assumption corrected in `ecbb0fe`.
The corrected selector test and three neighboring validation tests pass together
(six Qt Test rows including setup/cleanup). Across the 171-function inventory,
four functions remain explicitly skipped: native notifications require their
isolated test-bus runner, Wayland clipboard transport requires its separate
Wayland runner, and two lifetime stress functions are opt-in. Portable page-window
coverage passes on the owned Wayland backend;
actual Hyprland grouping is unqualified. These scopes are not summed into a
unique-test count or used to turn the original monolithic failure into a pass.

The isolated storage-only CPU comparison uses five alternating process pairs,
twenty page windows and identical twenty-second post-readiness idle periods.
The common harness, SDK and graphics facts match within every pair. Results are
in [the paired CPU artifact](storage-cpu-paired-20.json).

| Metric | Before | After | Paired mean-delta 95% interval | Decision |
| --- | ---: | ---: | ---: | --- |
| Main-process CPU | 43.834% | 43.471% | [-0.628, +0.194] percentage points | Inconclusive |
| Process-tree CPU | 43.916% | 43.532% | [-0.559, +0.258] percentage points | Inconclusive |
| Process-tree threads | 537 | 537 | [-1.8, +1.4] | Inconclusive |
| Process-tree PSS | 969,902 KiB | 972,986 KiB | [-4,099, +9,075] KiB | Inconclusive |

CPU is relative to one logical processor. These short post-readiness periods do
not establish several-minute steady-state CPU behavior. A separate 120-second
final-build twenty-window idle observation is retained as a current baseline;
its different interval is not used to claim a before/after CPU gain. Storage
scheduling is retained for its measured 95% reduction in idle timer checks and
bounded completion delivery, with no CPU, memory, thread or energy gain claim.


## Reproduction

Use `tools/performance/with_qt611.sh COMMAND` to select the qualified local SDK
without modifying the host. Override `FERRIC_QT_PREFIX`,
`FERRIC_KWINDOWSYSTEM_PREFIX` and `FERRIC_QTWAYLAND_PREFIX` when using another
qualified installation. Native application CMake additionally needs the explicit
`Qt6WaylandCompositor_DIR` from that matching QtWayland build.

Build the product and test harness from the source revision being measured.
`xtask perf` locates its sibling browser; keep both binaries together. Frozen
UI comparisons used one identical harness beside each independently built
browser. Their manifests distinguish invoking checkout provenance from product
and harness hashes. The entire frozen harness crate tree from `8f67af0` is
identical to the reachable `5dfc36e` tree; [the harness source receipt](harness-source.json)
records this equivalence. Later smoke-fixture changes do not change performance
suite code.

```sh
tools/performance/with_qt611.sh cargo build --release \
  -p ferric-browser-engine-qt -p ferric-browser -p xtask --locked

tools/performance/with_qt611.sh target/qt611/release/xtask perf \
  --suite core --samples 30 --output /tmp/ferric-core.json
tools/performance/with_qt611.sh target/qt611/release/xtask perf \
  --suite hints --samples 100 --output /tmp/ferric-hints.json

python3 tools/performance/benchmark_matcher.py \
  --baseline 6d02cfe642cdbdb21dd798d897643cafcaaee018 \
  --output /tmp/ferric-matcher.json
python3 tools/performance/benchmark_worker_first_use.py \
  --baseline 9ddc98d --output /tmp/ferric-worker-first-use.json

node tools/performance/check_hint_instrumentation.cjs
python3 -m unittest discover -s tools/performance -p 'test_*.py'
```

For UI measurements use an owned GL-backed nested Wayland session and a private
D-Bus session. This run used `weston --backend=headless --renderer=gl
--width=1280 --height=960 --socket=perf --idle-time=0 --no-config`, a fresh
`XDG_RUNTIME_DIR`, `QT_QPA_PLATFORM=wayland`, `QSG_RHI_BACKEND=opengl`, an empty
`QT_QPA_PLATFORMTHEME`, and `XDG_CURRENT_DESKTOP=weston`. The test-only helper is
built from `crates/ferric-browser-engine-qt/tests/performance-helpers` using
CMake. Pass its `plugins` directory to `--quit-plugin-dir`.

Run `xtask perf --suite ui --samples 30 --repetitions 2 --pages N
--presentation MODE --idle-seconds 10 --quit-plugin-dir PATH --output FILE` for
N=1/5/10/20 and MODE=internal/windows. The final idle observation uses 120
seconds. `benchmark_ui_pair.py` runs five alternating frozen-browser pairs;
select the same SDK/display environment before invoking it. Each tool's
`--help` lists its exact required arguments.

The QML runner is built from `tests/qml-runner` in the engine crate. Run it with
`-input .../tests/qml-perf/tst_hint_viewport_performance.qml` on the GL display
for the two-layout/one-layout comparison. The visible 500-marker baseline uses
`HintPerformance::test_T043_overlay_time_to_visible:overlay-500-100` with
`-input .../tests/qml-perf/tst_hint_performance.qml` and `QSG_INFO=1`.

`benchmark_tab_reconciliation.py` requires a separate owned checkout whose
controller still matches the pinned baseline and a separate candidate file.
It switches only that controller import, refuses conflicting edits, restores
it afterward and retains all six workload rows. The baseline controller hash
in the artifact also matches `6d02cfe`, so reproducing it does not require the
intermediate, superseded working commit named in the original run.

For native Switcher comparisons, build the C++ fixture against the exact Cargo
engine archive and generated includes from each qualified release build.
Separate worktrees keep the embedded-resource parity check meaningful for
both binaries. `benchmark_native_switcher.py` then runs five alternating
process pairs. `FERRIC_PERF_SWITCHER=1` opts into the history/timer fixture;
normal native test runs skip it. Select the archive/include paths from Cargo's
`--message-format=json` output, as the repository's native test builder does;
filesystem globbing can select a stale ABI. The measured manifests record
those selected paths and product hashes.

Artifact index: paired matcher, viewport, tab, worker-resource, native Switcher
and storage CPU comparisons contain raw samples plus retention statistics.
`storage-comparison.json` contains actual timer counts and completion samples;
`collector-baseline.json` and `decoder-baseline.json` preserve release baseline
samples. `matrix/` contains the exploratory complete 1/5/10/20-page resource
matrix. `final-overlay-gpu.json`, `graphics-runtime.txt`, the environment and
binary manifests record graphics/runtime provenance. `validation.json` is the
final qualification receipt; [native outcome excerpts](native-outcomes.txt) retain
initial failures, skips and bounded totals. `initial-qt612-validation.json` preserves the
superseded initial environment assessment.

The original monolithic `application_offscreen` attempt reached its 1,200-second
limit and included 124 `pageWindowPresentation` failures at the same
Wayland-tag expectation under an offscreen backend. These are recorded as a
qualification failure, not counted as passing. The dedicated owned-Wayland
portable page-window suite passed. Remaining offscreen-suitable application
functions are qualified separately in bounded process groups; backend-specific
window tagging/grouping remains outside this headless qualification.

## Qualification coverage

Performance workloads use controlled synthetic DOMs, disposable synthetic
history, actual production components and about:blank browser pages. They do
not establish live-site browsing speed. Main/tree CPU and endpoint memory are
measured; OS wakeup tracing, SQLite statement activity and physical power were
not collected. Startup/readiness observations are retained in the matrix;
restoration correctness is tested separately, with no claimed restoration or
window-switch latency improvement. The owned headless display does not qualify
actual Hyprland grouping or physical input.

## Remaining recommendations

Profile DOM visibility/hit testing with real complex applications before
attempting further changes. A future invalidation scheme must account for CSS
rules depending on arbitrary attributes and retain current activation preflight.
Investigate the baseline GPU multiwindow final Quit timer separately. Revisit
large reversed-tab reconciliation and maximum-size hostname policy indexing
only if user workloads show those paths are material. Collect process-tree task
accounting/wakeups and database tracing on dedicated hardware before making
energy, network latency or SQLite-activity claims.
