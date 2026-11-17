# Progressive hints implementation checkpoints

## Phase 0 baseline and feasibility

Starting production source: `277d72c74bb40d747ae8150f529b4f0eea2e7779`.
Production behavior is unchanged at this checkpoint. The live-input retry below
resolved the initial setup block. Phase 0 does not support proceeding with the
straightforward world-level publication cut under the plan's retention gate;
Phases 1–5 have not started. This report does not qualify a progressive feature.

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

### Stop decision

Do not begin provisional session state, pending action machinery or the new
scheduler on the strength of these results. The accepted execution plan requires
an early feasibility gate and meaningful measured benefit before retaining that
complexity. Its straightforward world-level early-publication cut does not meet
the proposed benefit threshold, and the tested discovery optimization regresses.
There is no useful production optimization from this experiment to retain.

This leaves the progressive feature **unimplemented**, with a completed negative
feasibility checkpoint for this specific cut. It does not establish that the
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
