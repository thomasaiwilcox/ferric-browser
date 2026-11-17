# Progressive hints implementation checkpoints

Current execution: the owner requested continuation after the Phase 0 stop
decision. The negative selector and late-publication measurements remain valid,
but they do not evaluate the full progressive pipeline. Implementation has
resumed with the Phase 1 state/readiness foundation below. Early publication is
not enabled and no end-to-end speedup is claimed.

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
