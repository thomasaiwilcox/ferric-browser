# Progressive hints implementation checkpoints

Final outcome (17 November 2026): the integrated prototype missed the live
benefit gate. After reviewing the measurements, the owner explicitly selected
**“Remove it per the plan”**. Provisional publication, pending-selection/readiness
state and prototype-only tests have been removed, including the Phase 1
foundation. Strict hints remain the production behavior.

The shared policy-assignment refactor, maintained feasibility tools and useful
child-observation test fixes are retained. Following a subsequent owner request,
the cleaned-up strict build was installed for testing and the owner reported
that it feels fast enough in use. The rejected prototype was never installed;
nothing was published or pushed. The chronology below records experiments, not
currently available progressive features; see [the final decision](#final-paired-measurements-and-removal-decision)
and [the subsequent installation](#subsequent-strict-build-installation-and-owner-feedback).

## Phase 0 baseline and feasibility

Starting production source: `277d72c74bb40d747ae8150f529b4f0eea2e7779`.
At the Phase 0 checkpoint, production behavior was unchanged. The live-input retry below
resolved the initial setup block. Phase 0 does not support proceeding with the
straightforward world-level publication cut under the plan's retention gate;
Phases 1–5 had not started. This report does not qualify a progressive feature.

### Controlled renderer measurements

The maintained feasibility fixture runs the real ordinary collector with a
deferred policy request, followed by real policy extraction. A separate native
CSS/viewport-only observation illustrates the proposed early data; it grants no
session, checked proof or action authority. Each fixture has 300 logical items
and 20 text spans per item. The collection query inspects all logical items.

Qt/WebEngine 6.11.2, Chromium 140.0.7339.225 (reported security patch
151.0.7922.71), offscreen/software rendering, 1000 × 700 viewport. Two warmups
followed by ten samples per fixture. The cheap observation always precedes the
strict collector; this can warm layout and is not an alternating implementation
comparison. Medians measure only renderer bodies, excluding native proof,
allocation, transport, input and presentation.

| Fixture | Viewport-only observation ms | Ordinary ms | Policy ms | Proposed / strict targets |
| --- | ---: | ---: | ---: | ---: |
| Grid | 3.90 | 53.35 | 59.10 | 30 / 30 |
| Feed | 3.45 | 50.15 | 51.05 | 10 / 10 |
| Covered grid | 4.40 | 53.65 | 60.65 | 30 / 29 |
| Covered feed | 3.40 | 49.15 | 56.80 | 10 / 9 |

All four fixture cases passed, plus setup/cleanup: six QtTest results, no
failures. Strict and proposed identities were stable across samples. The covered
target appears only in the cheap observation and is excluded by actual Ferric
admission. No navigation or activation is attempted by this diagnostic.

The covered-feed case crosses the default nine-character alphabet's capacity:
ten provisional bindings need two characters, while nine validated bindings can
use one. Reserving the false-positive code prevents later compaction. Therefore
this case must retain strict ordinary publication under the accepted capacity
rule. The grid cases do not cross that particular boundary; these counts alone
do not prove a general capacity solution or policy ownership independence.

Reproduce without rebuilding the native runner when it matches current native
sources:

```sh
python3 tools/performance/check_hint_progressive_feasibility.py \
  --output target/progressive-hints-feasibility
```

The runner defaults to `target/qml-tests/qml_tests`; use `--qml-tests` for an
existing qualified alternative. The command selects the pinned SDK. The output
contains the command, current revision, source hashes, individual rows and
summary in `result.json`, with full QtTest output in `fixture.log`.

### Dependency findings

| Fact | Current producer / consumer | Consequence for progressive publication |
| --- | --- | --- |
| Discovery and inventory | `PageInteractionScripts.hintCollector`, root/witness runtimes | Physical/root work remains required; cheap CSS results cannot replace bounded completeness |
| Eligibility and initial action meaning | Collector materialization and bounded naming | Deferred rejection is possible, but meaning used for a binding must already be captured |
| Membership and raw policy evidence | `hintFinishInteraction` | Collection numbers cannot be inferred from the cheap list of visible rectangles |
| Native graph seal and projection | Request adapter and frame broker | Unsealed observations must have a separate presentation-only type |
| Canonical policy ownership | `CollectionWork.finish` / `merge_worlds`, interaction proof | Existing code waits for all participating sealed worlds; an early callback alone cannot create a valid session |
| Label width | `CheckedHintCodebook.allocator` after ownership resolution | Complete bounded capacity planning or strict publication is required |
| Early input | Existing workspace reducer and Rust interaction | Reuse the queue before rows; never replay against a replacement allocation after proposals appear |
| Action permission | Existing native/Rust selection and activation routes | Readiness must be enforced below QML for every effect |

The first implementation should retain world-level validation boundaries.
Policy-dependent rows must wait until their binding facts settle; an early
ordinary row must have proven independent ownership. No new smaller policy
scheduler or second matcher is justified by these measurements.

### Accounting requirements before Phase 1

- Proposal storage and rejected bindings must fit current retention bounds.
  They cannot consume final admission capacity such that a later valid target
  disappears, or receive new inspection/candidate allowances per batch.
- Continuations preserve the original request's inspections, semantic
  predicates, annotation allowance and native verification grants. Native
  graph/document counters remain distinct from semantic predicate work.
- Separate continuation counts from the provider's bounded Ordinary/Policy
  semantic stages. Every continuation must progress and preserve cancellation.
- Geometry/style/hit facts expire across synchronous observation intervals
  unless dependency evidence establishes valid reuse. Mutation revisions alone
  cannot establish unchanged rank or geometry.
- Existing script deadlines and bounded settlement/recovery remain authoritative;
  chunks cannot each obtain a fresh request lifetime.

### Live setup failure and remaining exit gate

A separate owned normal-profile browser used the installed release
`6582e4785530301549137fd1d9f772c1f149cd12421e484cb4588a3d7fdf57e5`
with diagnostic resource copies and a temporary profile. Only site policies
were copied; no personal cookies, login session or userscripts were used.
Neither the installed executable nor personal configuration was modified.

The owned window was mapped but its native `active` property stayed false.
Attempts to focus it did not establish native keyboard ownership. All 24 initial
`f` baseline attempts failed to enter the normal measured journey; none is a
latency sample or product regression. AC power reported connected. Desktop lock
state itself was not established; an inactive window is not proof of a lock.

Raw setup attempts and controlled results are under ignored
`target/progressive-hints-20261117/`. The owner has been asked to unlock the
desktop and leave it available. Before Phase 1, obtain valid native-input
baselines and confirm useful policy-aware publication boundaries on YouTube and
Reddit. This is an outstanding Phase 0 exit check, not a failed progressive
performance gate. Do not claim an end-to-end speedup from the renderer table.

## Live retry and Phase 0 decision

The subsequent retry obtained real native focus. Session 1 reported
`Active=yes`, `State=active`, `LockedHint=no`, and the owned browser reported
`active: true`. The previous setup failure was not a product failure and did
not recur in the measured retry. All 48 recorded entries across two batches
completed; 24 used stage instrumentation and 24 used the ordinary timing path
in the diagnostic resource overlay. These latter rows still use the diagnostic
QML resource copies; they are not original-resource release qualification.

The same installed production executable was used with a fresh temporary profile,
copied interaction policies, 1280 × 720 viewport, 100% zoom, native Wayland and AC
power. Each batch includes three stage-instrumented and three ordinary timing
samples per case, with alternating instrumentation order. Batch 1 had one
unrecorded warmup per case; batch 2 had two. Startup/long-running outliers remain
in the data. These small samples support feasibility inspection, not tail-latency
or release qualification. No additional ten-pair candidate comparison is claimed.

| Case | Strict-path median ms batch 1 / 2 | Optimistic removable remainder ms batch 1 / 2 | Remainder as median fraction of instrumented journey batch 1 / 2 |
| --- | ---: | ---: | ---: |
| YouTube top | 372.6 / 367.2 | 66.0 / 59.8 | 16.3% / 15.7% |
| YouTube scrolled | 399.7 / 384.7 | 58.7 / 59.8 | 17.4% / 16.6% |
| Reddit top | 521.7 / 512.1 | 63.9 / 69.9 | 12.1% / 10.9% |
| Reddit scrolled | 567.8 / 561.2 | 72.7 / 74.2 | 12.6% / 12.4% |

The remainder is calculated separately for every instrumented sample as first
rendered-marker time minus the return time of policy extraction. The table
reports medians of those per-sample differences and ratios. It does not subtract
independent stage medians or divide a diagnostic remainder by a different
non-instrumented run. Both timestamps come from the diagnostic host clock.

This is a deliberately generous opportunity bound for the **existing schedule**:
it assumes labels can appear immediately at policy return, with zero new decoding,
allocation, overlay or frame cost. In reality ownership still depends on native
projection/merge, so an implementation needs additional presentation-only facts
and work. It is not a proof that a redesigned discovery/policy pipeline cannot
be faster. It does show that simply publishing ahead of sealing after the current
extraction cannot reach the proposed 20% early-label improvement on these samples.
The unchanged path spends most of its time before that cut.

Ordinary controls independent of policies could theoretically appear earlier,
but that is not evidence that the intended numbered video/post targets appear
earlier. The handoff expressly forbids claiming success from an unrelated badge
while the user's intended policy targets still wait. Early collection ranks may
not be guessed and repaired by moving displayed numbers to different items.

### Native selector experiment rejected

An ignored experimental source copy replaced the primary per-element selector
check with a per-root native `querySelectorAll` result set. It retained physical
traversal, eligibility checks and secondary discovery. This was a narrow
measurement prototype; it does not solve native query work accounting or all
custom-selector semantics and was never installed or added to production.

The maintained feed differential suite passed all four QtTest results, checking
six target families, three scroll positions, fresh documents and changed content.
The timing section used three warmup pairs and twenty alternating pairs per
position. Medians of ordinary renderer collection were:

| Scroll Y | Current ms | Native selector prototype ms | Change |
| --- | ---: | ---: | ---: |
| 0 | 90.05 | 93.65 | 4.0% slower |
| 1500 | 100.05 | 103.50 | 3.4% slower |
| 6000 | 96.75 | 104.60 | 8.1% slower |

Five fresh-page pairs likewise showed no consistent improvement. Matching
target fingerprints established equivalence only for the exercised fixture
cases. This is a policy-free renderer experiment, not an end-to-end/live or
custom-policy qualification. The clearly negative result rejects this particular
prototype without another batch. It does not reject every possible native CSS
selection design. No source swap, native rebuild or installation was required.

### Initial stop decision superseded by the owner request to continue

The initial decision was to hold provisional state and scheduling work because
the straightforward world-level cut did not meet the proposed benefit threshold
and the tested discovery optimization regressed. No production optimization from
that experiment was retained. The subsequent owner request to continue advances
the planned prototype; it does not turn the rejected selector experiment into a
beneficial change or establish a speedup for the complete design.

At that checkpoint the progressive feature was **unimplemented**, with negative
feasibility evidence for this specific cut. It does not establish that the
overall hybrid vision is impossible. A revised implementation would first need
to move the binding-determining policy/discovery work earlier or make it cheaper
while retaining complete membership, ownership, capacity and activation rules.
Showing speculative policy ranks before those facts settle would be a product
contract change, not a local optimization under the current plan.

Do not repeatedly tune the failed selector prototype or build a general
dependency engine to avoid reporting the stop gate. The maintained feasibility
fixture remains useful; the ignored prototype remains evidence only.

Evidence is under `target/progressive-hints-20261117/`: `baseline-batch1.json`,
`baseline.json`, `baseline-summary.json`, `retry-provenance.json`, and
`native-selector/{fixture.log,measurements.json,summary.json,control.js,candidate.js}`.
The original failed setup is retained under `attempt-1/`. Production source and
installed files remain unchanged. The owned browser is stopped and its temporary
profile, socket and debugging endpoint are removed at the end of the retry.

## Phase 1 state and readiness foundation

The existing `HintInteraction` now owns provisional readiness and at most one
pending label selection. The existing codebook, bindings, input grammar and
allocator remain the owners of those concerns. There is no second input queue,
policy matcher or label allocator.

The additive core APIs are `begin_hint_validation`,
`new_provisional_enhanced`, `settle_initial_hint_reservations`,
`apply_hint_validation`, status/pending accessors and explicit cancellation/close.
They are native caller APIs, not renderer-callable proof constructors. An update
is matched to its allocated label, physical identity and current action meaning;
changed geometry must still be finite and valid. Batches are sequenced and
checked atomically before promotion. Empty nonterminal batches, duplicates,
unknown bindings, changed meanings and premature completion are rejected.

Initial omitted special declarations have a distinct `PendingInitial` state.
They can settle once during initial collection, either unavailable or bound to
a new provisional target. They cannot claim an already represented ordinary
target. The caller supplies complete ordinary capacity before allocation;
retained bindings, new special rows and their meanings retain the existing
explicit payload budget. A missing or retired special code cannot acquire a new
target through passive refresh.

Full-code input or explicit confirmation of a provisional row records one pending
selection and returns an editing update. Validation can return that exact label
once; it does not create an action permit. Editing, query changes, cycling,
replacement selection, cancel and request close discard it. Completion without
an explicit pending selection never produces activation. Adapter-owned absolute
expiry and document/view cancellation still need their producer integration in
later phases.

Native selection, palette action entry and pointer confirmation now consult
readiness before consuming action state. Policy anchor capture also refuses
unvalidated bindings, and frozen anchor currentness checks readiness. Existing
freshness, document, scope and action-permit checks remain in place. Internal
interaction diagnostics expose pending validation/selection; the public policy
configuration schema is unchanged.

Production collection still creates completed strict interactions. No code path
publishes provisional rows yet. Renderer continuation, proposal transport,
same-session final publication and new native lifecycle journeys remain later
work; this checkpoint does not claim them as implemented or tested.

### State validation

`cargo test -j 2 -p ferric-browser-core --offline --locked` passes all 245 tests:
193 library, 11 codebook, 26 existing enhanced-interaction, 12 new progressive,
and three mode-entry cases. The new cases exercise exact pending selection,
one-time settlement, terminal missing reservations, withdrawal without code
reuse, atomic rejected batches, changed identities/action meanings, invalid
geometry, cancellation/close, retained-budget failure and a complete 5,000-row
batch. This is correctness coverage, not reopening stress-load latency gates.

An initial implementation changed a legacy ordinary auto-follow case; that
regression was corrected before the passing full run. Strict ordinary behavior
remains covered by the original tests. `cargo clippy -j 2 -p ferric-browser-core
--offline --locked --all-targets -- -D warnings`, workspace formatting checks and
`git diff --check` pass. Logs are in ignored `target/progressive-core-tests.log`,
`target/progressive-core-clippy.log` and `target/progressive-format.log`.

The qualified Qt 6.11.2 engine check passed with
`tools/performance/with_qt611.sh cargo check --release --offline --locked
-p ferric-browser-engine-qt --lib`. The first invocation selected a separate
check artifact set and rebuilt unchanged Qt/C++ bindings, adding avoidable build
time (24m 32s). A subsequent check of the final core source reused those artifacts
and passed in 35s. These are compile checks, not a linked release browser or new
native runtime journey qualification. Logs are
`target/progressive-engine-check.log` and
`target/progressive-engine-check-current.log`.

### Input and retained state comparison

An isolated optimized comparison builds the original core from `277d72c` and the
current core into one diagnostic executable. It does not swap production files
or modify source timestamps. Each sample processes 100,000 character/backspace
pairs over 30 ordinary targets with automatic following disabled. Two warmup
pairs precede ten alternating baseline/candidate pairs. Measurements run after
the native build finishes.

| Metric | Original core | Current core |
| --- | ---: | ---: |
| Inline `HintInteraction` size | 504 bytes | 544 bytes |
| Accounted retained payload for 30 identical enhanced bindings | 5,811 bytes | 5,811 bytes |
| Median time per input operation in this loop | 384 ns | 374 ns |

The small timing difference is not a speedup claim. The strict path keeps a
direct fast path into existing input handling. Per-binding readiness uses
existing layout padding in this build; the one optional progressive record adds
40 inline bytes per interaction. Accounted payload is not allocator/RSS or total
working-set measurement. This comparison excludes DOM collection, Qt publication,
transport and real keyboard latency, and establishes no end-to-end benefit.

Reproduction inputs, individual samples, source hashes and summary are under
ignored `target/progressive-hints-core-bench/`. Its control source is extracted
from the named original commit; `src/main.rs`, `Cargo.toml`, `samples.log` and
`result.json` identify the complete diagnostic. Run
`cargo run --release --offline -j 2 --manifest-path
target/progressive-hints-core-bench/Cargo.toml` after preparing those recorded
inputs. The essential results above remain versioned for other checkouts.

This retains the Phase 1 foundation as supporting implementation work. It does
not close the later producer, renderer continuation, native lifecycle or visual
qualification gates. The rejected native selector remains rejected. No browser
installation, PR or push is part of this checkpoint.

## Phase 2 shared policy calculation

Anchor claims, collection ordering and ownership assignment now run through one
private read-only `BindingView` over the normalized graph, policy and canonical
candidate snapshots. `CheckedAnnotations.special_assignments` delegates to it.
Collection freshness comparisons remain on the checked-proof type and retain
their original native associations and exact witness comparisons.

This separates policy interpretation from the checked-proof wrapper so that the
proposal producer can reuse the same calculation. It introduces no second
matcher, unchecked-to-checked conversion, authority token, new wire version or
renderer result. The view only borrows three existing data structures and adds
no retained inventory or cache. The assignment and ordering algorithms are moved
without changing their behavior. Provisional publication is still unwired.

The failed native-CSS prototype remains excluded. Ordinary JavaScript discovery
is unchanged in this checkpoint. No speedup or completed progressive pipeline is
claimed for this supporting refactor.

### Contract tests without Qt linking

`tools/qualification/check_hint_contracts.py` creates a small isolated Cargo
harness under `target/`, links the actual production module files and executes
their existing tests. It includes payload decoding, policy/proof interpretation,
root and world merging, fuzz regressions, request work sequencing and native-work
ledger unit contracts. It does not duplicate test bodies, provide a replacement
policy engine or qualify real native receipts. The native input-size declaration
comes directly from the production source; dependencies start from the production
lock file and the helper records source and lock hashes.

The first harness attempts exposed Rust module lookup and a missing application
test dependency. Those were harness setup errors. The final harness preserves
the original module/fixture layout and includes the actual application crate;
it does not rewrite production or test logic to work around errors. Subsequent
attempt logs are archived under the output directory.

Run:

```sh
python3 tools/qualification/check_hint_contracts.py \
  --output target/progressive-hints-contracts
```

Final result: **188 tests passed, zero failed** in 12.20s after compilation. One
existing documentation example is ignored separately; no unit test is ignored.
The results cover ambiguity, anchor ownership, collections and ordering, graph
and candidate association, malformed input, shared budgets, one-shot grants and
work sequencing. The harness avoids Qt linking; these are not native window,
keyboard or runtime performance results.

The current policy-view source also passes the qualified Qt 6.11.2 release engine
compile check in `target/progressive-policy-view-check.log` (23.53s using the
existing artifacts). Workspace formatting, Python compilation and diff whitespace
checks pass. This checkpoint changes no dependencies of the shipping browser.

Raw test output and source provenance are under
`target/progressive-hints-contracts/`. The retained report and reusable command
allow another checkout to run these same source tests. Proposal transport,
publication and native end-to-end qualification remain outstanding.

## Phase 3 initial integration — evaluated and removed

The first integration prepares a complete policy-resolved presentation for a
single Top document with at most 64 ordinary candidates, before native graph
sealing. This is one bounded validation unit, not a new candidate allowance.
Foreign/same-origin child-document inventories, label proxies, larger inventories, scopes,
pointer/search/rapid/first invocations and passive refresh retain strict
publication. The temporary internal environment variable
`FERRIC_HINT_PROGRESSIVE_EXPERIMENT=1` selects this prototype; the shipping default
and installed browser are unchanged. It is not a public mode or policy setting.

The producer uses the existing ordinary inventory and canonical world merge,
including document/root namespace remapping. It then runs the same normalization
and assignment calculation. The raw original graph remains available for native
sealing. The canonical result is retained and reused after sealing, rather than
repeating semantic matching with a new budget. Its structural verification work
is charged before subsequent native grants. Failures cannot refill or retry
normalization within that request. Native completion must corroborate the same
Top viewport, without excluded/limited/foreign projections.

Native staging alone can create the typed presentation. Renderer/QML envelopes
cannot opt into provisional publication or grant readiness. QML reads the
existing Rust session for display; initial early-input gestures are replayed once
against that allocator. Completion updates the same interaction and session ID,
withdraws any disproved bindings, and returns an explicit pending selection once
to the existing fresh action route. Completion without prior selection does not
auto-follow. A failed displayed proposal closes instead of silently reallocating
its codes on the ordinary automatic retry.

The pure contract harness passes **194 tests**, including exact canonical output
and special-assignment comparisons with strict merge for anchors and collections,
refresh identity, seal/projection refusal, shared one-shot accounting, unchanged
labels and explicit-only pending selection. These tests do not qualify native
publication, rendering, keyboard focus or performance. The compiled native
application fixtures and paired first-policy-render measurements are the next
qualification step; no speedup or release-readiness claim is made yet.

The maintained application tests include `hintProgressivePublication` and the
opt-in `hintProgressivePerformance` comparison. The latter reuses the existing
render-signal observer, real input router, production resources, collector,
policy and native continuation. It compares two warmup pairs and ten alternating
strict/progressive pairs on fresh grid/feed documents at two scroll positions.
It records useful policy-label rendering separately from validation completion,
checks unchanged initial/final labels, and retains full candidate projections.
It does not measure compositor presentation or completed-action latency.

### Initial native trial and measurement-fixture correction

The first optimized engine build completed in 20m33s with two build jobs and the
qualified SDK. The first non-injected application trial passed six production
workspace cases (eight QtTest results including setup/cleanup, zero failures,
14.762s): provisional display, explicit confirmation, editing, text query,
Escape/cancel and explicit cancellation of pending intent. Each provisional
publication attempted the ordinary native selection route and nine action/palette
routes directly; all returned `hint-validation-pending`. Confirmation executed
once; the other cases executed no action. Final labels and the session ID were
unchanged. These are offscreen functional results, not latency measurements or
physical input qualification. The final ownership/accounting refinements and
additional early-input cases require the subsequent clean rebuild and rerun.

The first grid render comparison correctly failed its requirement to observe
code `1`: the benchmark configured its collection for family `all`, whereas the
actual default `f` binding invokes `actions`. Ordinary labels appeared, with no
numbered reservations. The fixture now configures `actions`; the policy-label
assertion remains. No timing sample from that failed attempt is retained as a
progressive result. Logs are `integrated-grid-comparison.log` and
`grid-diagnostic/states.json` under the ignored evidence directory.

The one diagnostic retry loaded an existing external generic Qt inspection
plugin to read the runtime state. That process exited with SIGSEGV after the
QtTest summary, in `QGuiApplicationPrivate` / `QApplicationPrivate` destruction
through a null call. Other threads showed idle WebEngine/IPC workers; no OOM
was recorded. The same test without injection exited normally (with its fixture
assertion failure), and the non-injected publication trial also exited normally.
This correlates the shutdown failure with the injected run but does not prove
the precise offending function. It is excluded from performance qualification;
subsequent qualification uses the maintained non-injected executable. The
extracted temporary core was deleted. No installed browser or personal profile
participated. The diagnostic backtrace is `grid-diagnostic/crash.txt`.

### Controlled render and action comparisons

The non-injected final-source publication trial passes eight journeys (ten
QtTest results, zero failures, 19.772s), including query and confirmation gestures
queued before any labels exist. It preserves a single session ID and allocator.

The initial render comparison used the render observer's 5ms event loop until
first labels, then QtTest `qWait` polling until validation. That changes the host
waiter earlier in the progressive case and produced a large apparent completion
penalty. The observer now retains the same event-loop waiter through validation
in both controls, while keeping the original first-frame timestamp. Both events
use the native steady clock. The mixed-wait run remains diagnostic evidence in
`final-comparison-batch1.log` / `mixed-wait-summary.json`; its completion numbers
are not an architecture comparison.

The first neutral-wait batch uses two warmup pairs and ten alternating pairs on
fresh documents, Qt 6.11.2, offscreen/software rendering, a 1000×800 window and
1000×774 page viewport, zoom 1, AC connected. It observes the actual rendered
text for policy code `1`, not merely an ordinary label or a published model.
All 40 paired final target/code/geometry projections match exactly. Every
provisional initial/final Hint JSON array is identical: no withdrawn targets,
retargets, changed ordinary code lengths or changed numbered assignments in
these unchanged fixtures.

| Fixture | Strict first policy frame ms | Progressive ms | Earlier ms / % | Strict / progressive validation complete ms |
| --- | ---: | ---: | ---: | ---: |
| Grid, top | 393.02 | 308.64 | 84.38 / 21.5% | 385.38 / 416.97 |
| Grid, scroll 2100 | 380.59 | 295.65 | 84.94 / 22.3% | 373.53 / 399.98 |
| Feed, top | 341.52 | 261.34 | 80.18 / 23.5% | 336.29 / 349.69 |
| Feed, scroll 2100 | 338.32 | 265.62 | 72.70 / 21.5% | 332.29 / 351.26 |

Early presentation has a real validation-completion cost here, even after the
waiter correction. These are one-batch medians, not tail estimates, compositor
presentation, live-site qualification or evidence that completed actions are
faster under every input schedule. Raw data and exact comparisons are in
`neutral-comparison-batch1.log` / `neutral-comparison-batch1-summary.json`.

The maintained action comparison uses the same DOM, replacing primary links
with non-navigating buttons. It sends constructed native code-1/Enter events at
fixed offsets from `f`, observes the exact `target-0` DOM activation, and checks
that it executes only once. A separate schedule sends input 100ms after the
first rendered policy frame; it is reported separately from fixed input timing.
Both use the native steady clock and identical 5ms asynchronous receipt polling.
The initial coarse-timer action run exposed unequal actual delivery offsets, so
qualification uses precise timers and records actual selection timestamps.

In the first precise batch, selection following visible labels completed 82.57ms
earlier on the grid and 110.21ms earlier on the feed. Blind 100ms input showed
larger slowdown in that batch (including a late-run shift affecting both
controls), so those cases were repeated without changing production code. The
repeat medians were 745.54 / 770.20ms (strict/progressive grid) and 663.45 /
673.86ms (feed): 24.66ms / 3.3% and 10.41ms / 1.6% slower respectively. Thus the
large first-batch slowdown has not reproduced above both proposed regression
thresholds. The validation overhead remains real and the remaining repeat/live
checks are still required; these results do not qualify every early input case.

### Final-publication reentry qualification

Two additional deterministic native cases edit or restart Hint mode from the
final publication notification. Both fail on the first integrated prototype:
the pending-selection dispatch can still use its old reply after UI publication
has reentered input handling. These failures are retained in
`publication-reentry-red.log`; they were found before enabling or installing
this feature. The dispatch now also checks the original collection epoch,
session ID and the exact input-state object. An edit replaces that state object;
a restart replaces the owning epoch/session. No second intent queue or action
permission is introduced. The fresh production-resource build passes both cases. All ten publication
journeys pass (12 QtTest results including setup/cleanup, zero failures,
23.299s). `publication-final.log` records the result;
`lifecycle-final-provenance.json` records exact production-source and native
artifact hashes. Neither source timestamps nor archive freshness checks were
bypassed. This completes the deterministic publication reentry check; broader
lifecycle and performance qualification continues below.


### Broader regression run

The first broader offscreen regression run overlaps the compiler (functional
checks only; no performance samples are taken under build load). One existing
`hintPageKeyRouting(stale-refresh)` assertion missed its unchanged 5s check;
QtTest reports it would pass at 5100ms. Its failure and state diagnostics remain
in `regression-final.log`. It needs an isolated enabled/disabled repeat after
compilation; build contention is a hypothesis, not an established cause. The
operation and test timeouts have not been increased.


The run finished with **266 passes and six failures** (1045.596s). Five
`hintRequestObservationDocuments(work-policy-*)` cases fail before policy
verification, while waiting for an initial child ResizeObserver invalidation.
All five reproduce with the experiment disabled, at the identical assertion,
in `regression-baseline-failures.log` (three passes, five failures, 26.475s).
That disabled repeat passes stale-refresh. These comparisons do not establish
that the child-observation tests pass on the earlier committed baseline; they
show the failures are independent of selecting the progressive branch in this
binary. Owned Wayland qualification and an idle stale-refresh repeat follow.


Owned Wayland repeats all ten publication journeys, stale-refresh and the
two-tab media-key journey successfully (14 passes); the same five child setup
assertions fail (five failures, 80.518s total). This rules out an offscreen-only
explanation. Source history identifies the obsolete assumption: commit
`066cf8b` already suppresses unchanged initial ResizeObserver reports. The
positive adapter fixtures now wait for observed initial-report delivery itself,
without demanding a dirty revision. The stale-child negative makes a real
layout change and checks a dirty newer revision before the existing rejection
assertions. The retained-nonce refinement below corrects a second older test
assumption about notification-driven inventory cancellation. No production
collector, deadline or authority assertion changes for this test repair. The
original enabled/disabled failures remain available for comparison.


### Qualification setup and lint closeout

The engine passes release Clippy with `-D warnings` using the qualified SDK.
Three reported style lints were fixed (two redundant closures and one wildcard
covering a single variant). The functional production behavior is unchanged;
`release-final-inputs.json` records these two Rust-file differences from the
native artifact used above. A fresh release build is required before calling
that final source qualified. Existing GCC/Qt header diagnostics remain in the
build output; the Rust lint run exits successfully.

`hintEarlyInput` and `hintNativeControls` pass all 32 offscreen results,
including primary/secondary windows, navigation, view/focus changes, shutdown,
query ownership, early confirmation and pointer behavior (145.137s).

The first live run is **setup evidence, not representative performance
qualification**. The disposable YouTube profile opens a consent screen, so it
has no numbered video bindings. On native Wayland the compositor tiles the
requested 1280px startup window to 640×694 logical pixels (640×668 page, DPR
1.5). Reddit's top case completes ten matched pairs with unchanged projections,
but a cookie overlay remains and the scrolled case has no numbered post label.
Its measured 22.13ms / 10.6% earlier first policy frame is retained as diagnostic
evidence; it does not pass the primary-workload gate. No failed first-label
attempt is counted as a timing sample.

The maintained live harness now requests application fullscreen, records the
actual page viewport and can explicitly reject optional-cookie consent in its
own disposable profile before measurement. Its optional ready-selector checks
are fixture parameters. This setup code is not installed in the collector or
policy engine, and does not change personal cookies or any website's production
handling. Original logs are `live-youtube-batch1.log` and
`live-reddit-batch1.log`.


### Historical owner trial guide — prototype removed

This was the proposed review guide. No candidate was installed or delivered for
this trial; the owner subsequently chose removal. It is retained as part of the
experiment record, not as instructions for the current checkout.

1. On YouTube search/home and watch pages, open hints and start reading/typing
   immediately. Check numbered videos and named anchors, not just ordinary links.
   A displayed code must keep its target; it may disappear if that target changes.
2. Select immediately, then repeat after reading the labels. Each selection
   should perform one expected action. Repeat Enter must not duplicate it.
3. Type part of a code, edit it, switch to `/` query input and cancel with Escape.
   Wait briefly after cancellation; no delayed action should occur.
4. Open a second YouTube tab. On both tabs try `f`, then `p` (and fullscreen).
   A successfully delivered shortcut returns to Normal after release. Ordinary
   Normal-mode input must not inherit the Hint mapping.
5. Navigate from YouTube to Reddit, scroll several screens, and repeat `f` and
   selection. Revisit the other tab and repeat. Check that numbered posts remain
   available and no old tab's selection executes.
6. While hints are collecting, switch tabs/windows or focus a browser prompt,
   then return. An old selection must not execute after that ownership change.

Judge whether labels become useful earlier without distracting corrections,
longer codes, missing controls or delayed surprises. Timing alone cannot pass
this review. The installed release remains unchanged until delivery is explicitly
requested; the experimental path remains off by default during qualification.


The additional lifecycle/refresh matrix passes **21 results** (79.787s):
navigation invalidation, embedded-resource parity, actual hint keyboard
activation, input-preserving refresh, no refresh auto-follow, confirmation
freezing and scoped Hint/Target Search journeys. The final-source pure contract
harness also passes **194 tests**. Source/lock hashes are retained by the harness.


The final release build completes in 18m04s with two jobs. All recorded
production-source hashes match its inputs. The native archive SHA-256 is
`def7c5007a32c6a9758c6f5c5cd1dc25ce22bdc8afaac6455246103c2279f3bd`;
the standalone browser SHA-256 is
`a43f008f0d137cc4e7fae52efb3cf92e2f5acd82552a1fa2e1a1a4093a4f1bb2`.
CMake's archive freshness check passes before relinking the application runner.

The final-source Wayland run passes 19 results, including all ten progressive
publication journeys, stale-refresh, two-tab media keys, resource parity and
four repaired child cases. Its only remaining failure identifies a second
pre-existing test assumption: `9d39e43` retains the weak pending inventory on a
dirty notification instead of deleting it. The changed-child fixture now checks
that the original nonce remains retained, then still requires policy completion
to reject the changed snapshot, clear the native work, grant no checked
observation, preserve the displayed snapshot and execute zero clicks. All five
child cases then pass (seven results including setup/cleanup, 17.569s).
`release-final-native.log` retains the intermediate failure;
`child-repair-final.log` records the repaired cases. These are test repairs for
committed behavior; production deadlines and collector behavior are unchanged.

## Final paired measurements and removal decision

The final controlled repeat passes all 12 results (231.270s). It uses the final
prototype source/resources, two warmup pairs and ten alternating pairs per case,
with the neutral waiter and precise action timers described above. Every one of
its 40 paired final projections matches; all displayed initial/final arrays are
unchanged. The first controlled batch also had 40 exact projection matches.

| Controlled fixture | Strict / progressive first policy frame ms | Earlier ms / % | Validation completion cost ms |
| --- | ---: | ---: | ---: |
| grid, scroll 0 | 389.14 / 304.01 | 85.13 / 21.9% | 30.67 |
| grid, scroll 2100 | 383.36 / 301.91 | 81.45 / 21.2% | 35.40 |
| feed, scroll 0 | 340.69 / 256.45 | 84.25 / 24.7% | 13.03 |
| feed, scroll 2100 | 341.20 / 259.19 | 82.01 / 24.0% | 10.94 |

Completed actions in the repeat use the exact same input schedule within each
comparison. The after-label schedule starts 100ms after each control’s own first
policy frame; it must not be conflated with input fixed relative to `f`. All
observed actions activate `target-0` exactly once and return to Normal.

| Fixture / input schedule | Strict / progressive action receipt ms from `f` | Progressive difference ms |
| --- | ---: | ---: |
| grid / 100ms after f | 742.81 / 763.13 | +20.31 |
| grid / 500ms after f | 863.69 / 859.99 | -3.70 |
| grid / 100ms after labels | 854.98 / 779.23 | -75.75 |
| feed / 100ms after f | 649.71 / 658.91 | +9.20 |
| feed / 500ms after f | 825.20 / 828.74 | +3.54 |
| feed / 100ms after labels | 754.93 / 689.41 | -65.52 |

The consent-cleared, settled Reddit runs use native Wayland, software rendering,
Qt 6.11.2, AC power, zoom 1, a 1280×694 logical page viewport and DPR 1.5. Each
case has two warmup pairs and ten alternating pairs. The existing live baseline’s
two-second page-settlement interval precedes T0 and all warmups. These measure a
swapped Qt frame, not compositor presentation.

| Live Reddit case | Strict / progressive first policy frame ms | Earlier ms / % | Validation completion cost ms |
| --- | ---: | ---: | ---: |
| Batch 1, scroll 0 | 229.51 / 209.32 | 20.19 / 8.8% | 48.72 |
| Batch 1, scroll 1800 | 485.46 / 435.74 | 49.72 / 10.2% | 31.03 |
| Batch 2, scroll 0 | 235.28 / 205.01 | 30.27 / 12.9% | 36.12 |
| Batch 2, scroll 1800 | 473.60 / 452.27 | 21.33 / 4.5% | 52.05 |

All 40 live Reddit candidate/control projection pairs match exactly. All 40
candidate attempts use the prepared single-Top path (zero strict fallbacks among
these completed samples); initial/final displayed arrays remain identical.
This establishes neither a general fallback rate nor live completed-action
latency. The live relative benefit does not reproduce the proposed 20% gate;
validation is 31–52ms later. Controlled action gains cannot be extrapolated to
these live pages.

YouTube has **no qualified ten-pair batch**. The logs distinguish consent and
configured-primary readiness failures from collection staleness after page
loading/scrolling. In one trial policy labels reach the frame observer before
collection closes; this is an observed withdrawal/failed journey, not a successful
speed sample. Later settled trials still fail. These results do not establish
whether each failed collection actually selected the progressive branch, or
whether the strict branch would fail at the same page-load instant. No positive
YouTube speed or coverage claim is justified. The second YouTube batch was not
run after its first batch failed; failures were not discarded or replaced.

The owner reviewed the live benefit/cost and explicitly chose **“Remove it per
the plan”**. The Phase 3/4 publication prototype and the Phase 1 core readiness,
pending-selection and PendingInitial machinery are removed. Prototype-only
native/pure tests, its benchmark fixture, UI callback and temporary environment
switch are removed too. No provisional contract is left in the current
architecture/user documentation. Strict collection remains authoritative for
all contexts, without changed eligibility, policy semantics, work allowances,
frame deadlines or the deferred 5,000-target case.

Retained work is the shared policy-assignment interpretation in `275b5fb`, the
maintained Phase 0 feasibility fixtures/tools and the child-observation test
repairs for already committed behavior. The shared refactor is retained for
one interpretation of the policy graph, not advertised as a performance gain.
The original installed release remains untouched. No human trial, default
rollout, installation, PR or push was performed for the rejected prototype.

Raw evidence is ignored under `target/progressive-hints-20261117/`:
`qualified-live-{youtube,reddit}-b*.log`, their summary JSON, and
`final-controlled-batch2.log` / `.summary.json`. The rejected source/resources,
runner and standalone binary are identified by
`rejected-prototype-provenance.json`; `removed-prototype.patch` and the copied
fixture preserve the local experimental inputs. They are historical research
artifacts, not installation candidates. If those local artifacts are absent in
another checkout, use this report as summarized evidence; the current checkout
intentionally cannot run the removed prototype benchmarks.

The post-removal contract harness passes **188 tests** using actual retained
production modules. Core tests, formatting and final source review are recorded
below. The prototype’s successful native build/tests above qualify that measured
prototype; they are not a claim that a new post-removal release was built. The
retained production code returns exactly to the committed strict implementation,
plus the already qualified shared policy-view refactor. No replacement native
binary is supplied for this removal; a fresh source build is required before
any future packaging or installation.


### Retained-tree validation and local commits

- Child-observation fixes: local commit `24c8c58`. All five repaired cases pass
  on owned Wayland (seven results); the changed-child negative also passes with
  the experiment disabled (three results, 3.743s), retaining its stale-rejection
  and zero-effect assertions.
- Post-removal core tests: **233 pass** (193 unit and 40 integration tests). Core Clippy passes for all targets with `-D warnings`.
- Post-removal production-module contract harness: **188 pass**. Reproduce with
  `python3 tools/qualification/check_hint_contracts.py --output target/hint-contracts`.
  This remains a Qt-free logic check, not a substitute for native qualification.
- Workspace formatting, diff whitespace and KDE-free schema-v2 policy checks
  pass. Dependency-policy tooling's 15 tests also pass.
- Exact source checks confirm every removed foundation file matches `6c4563d^`
  (or is absent if it was introduced there), and the retained interpreter files
  still match `275b5fb`. The only other crate change is the reviewed test repair.
  `retained-source-review.json` records this check.
- The installed executable still hashes to
  `6582e4785530301549137fd1d9f772c1f149cd12421e484cb4588a3d7fdf57e5`.
  Rejected prototype artifacts are explicitly marked not for installation.
  No post-removal full native rebuild or whole-workspace CI pass is claimed.

The retained scope is complete. Further progressive work requires a new owner
request and a design that earns its cost on representative native workloads;
there is no pending rollout or permanent experimental switch in this checkout.


## Subsequent strict-build installation and owner feedback

At the owner's subsequent request, clean source `8c27c75a371861dc7e5040fd09226b0461a95486`
was rebuilt using Qt 6.11.2 and two build jobs. The release build passed in
16m31s, with identical source manifests before and after. CMake's archive
freshness check passed. The fresh application runner passed all **16 targeted
checks** (zero failures/skips, 47.786s), covering early input and selection,
policy actions, two-tab page-key routing, refresh, resource parity, keyboard
activation and the repaired child-observation cases. These application checks
used the offscreen software backend.

The installed executable SHA-256 is
`0f374110d84b4a6e576fe0ebf63a1fefd883b62b7378c2ed6015154f02472e38`,
in release `strict-hints-0f374110d84b4a6e`. Its launcher passes `--version` and
`--help`. A separate installed-product check on native Wayland, using a
disposable profile, verifies the actual executable, a mapped window, a healthy
complete active tab, the exact source manifest and the KDE-free runtime/library
closure. Personal profiles and browser defaults were not changed. The previous
`6582e478` release is retained, with guarded rollback metadata. Evidence is under
ignored `target/strict-hints-install-20261117/`, including `build.json`,
`hint-smoke.log`, `installed-startup.json`, `installed-runtime.json` and
`complete.json`; the runtime's `installation.json` records the active release
and rollback command.

The owner subsequently reported that hints feel faster, though still slower
than qutebrowser, and fast enough for practical use. This is positive hands-on
feedback, not a new comparative benchmark or a claim about wider user
satisfaction. The earlier measured sparse-inventory optimisation remains in
the installed code. No additional measured speedup is attributed to the shared
policy refactor or progressive-state removal, and the rejected progressive path
remains removed. No further performance tuning is planned under this task.
