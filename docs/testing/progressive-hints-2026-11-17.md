# Progressive hints implementation checkpoints

## Phase 0 baseline and feasibility

Starting production source: `277d72c74bb40d747ae8150f529b4f0eea2e7779`.
Production behavior is unchanged at this checkpoint. Phases 1–5 have not started;
the live production-input baseline is outstanding. This report does not qualify
the proposed progressive implementation.

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
