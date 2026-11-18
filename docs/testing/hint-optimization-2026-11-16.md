# Hint collection optimization — 16 November 2026

Baseline: `066cf8b`, Qt 6.11.2. Changes are measured independently and retained
only after correctness and repeated end-to-end benefit. No public interface or
policy behavior changes are intended.

Phases: classification; retained inventory; geometry; predicate reuse; native
verification. Each has a keep/revert gate: at least 5% and 10 ms median benefit
on a primary workload, reproduced in a second batch, with no material regression.
Primary cases: YouTube linux search and Reddit popular, top and 1,800 px scroll.
The initial paired experiments used the tiled 640 × 694 viewport, 100% zoom;
these differ from the earlier full-width profiling workload.
Two warmups and ten alternating baseline/candidate pairs per case; at most one
additional batch for inconclusive results. Diagnostic counters are separate.

Evidence and diagnostic sources: ignored `target/hint-opt-20261116/`.

## Outcomes

### Phase 1 — classification: reverted

Ten alternating pairs per case, all with matching ordinary and displayed target
fingerprints. Baseline/candidate medians (ms): YouTube top 332/335, scrolled
317/318; Reddit top 488/508, scrolled 430/425. No case met the retention gate.
The candidate hoisted family/policy decisions and reused ordinary eligibility;
it was removed rather than adding an unmeasured cleanup to the shipping code.

### Phase 2 — sparse retained inventory: provisionally kept

Text/comment positions remain null slots: physical inspection/search positions
and limits are preserved, while locations, parent validation, and WeakRefs retain
only elements and required roots. Name/mutation evidence remains live.

All 56 deferred-collection cases passed, including new sparse-index and text
mutation checks. Both live batches had ten matched pairs per case, zero target
fingerprint mismatches, and zero collection failures.

| Case | Batch 1 baseline/candidate ms | Batch 2 baseline/candidate ms |
| --- | ---: | ---: |
| YouTube top | 323/310 | 317/295 |
| YouTube scrolled | 302/283 | 301/263 |
| Reddit top | 458/455 | 211/203 |
| Reddit scrolled | 416/383 | 458/419 |

Different page loads had different DOM sizes (notably Reddit top); compare only
within each matched batch. Scrolled improvements reproduced above the gate.

### Phase 3 — scrollability before rectangles: reverted

Against the accepted inventory version, ten matched pairs each gave medians
(ms): YouTube top 294/340, scrolled 262/316; Reddit top 261/287.
The Reddit scrolled baseline warmup failed collection during feed changes;
no fourth-case timing claim is made and the failure remains in the raw log.
The completed cases clearly rejected the candidate without another batch.
Separate counters confirmed the tradeoff: Reddit top rectangle calls dropped
2,467 to 232, but computed styles rose to 2,503 and the ordinary body slowed
116 to 146 ms. These instrumented single samples diagnose the work shift,
not production timing percentages. All geometry changes were removed.

### Phase 4 — predicate cache: skipped at evidence gate

A separate isolated-realm diagnostic counted element/compound pairs within each
policy extraction. YouTube top/scrolled: 11,035/11,030 evaluations; Reddit:
8,657/8,658. All four had **zero repeated pairs**. A cache cannot reuse these
results and would add allocation/lookup work. No cache was implemented.

### Phase 5 — verification: skipped at complexity/safety gate

Reviewed captureFragmentCatalogue, revalidateDocument,
revalidateFragmentCatalogue and projectFragmentCatalogue. Recapture compares the
exact original snapshot after asynchronous document checks; projection captures
geometry before and after hit testing. They establish different freshness
boundaries. No safe local consolidation was identified. Existing stage timing
covers verification as a whole; no new per-probe native timing implementation
was added because that would not justify removing any of these boundaries.
This is a deliberate stop-gate outcome, not a measured verification speedup.

### Validation adjustment — existing identity tests

The broad HTTP run found three document-binding assertions failing identically
with the original runtime. The current native contract explicitly validates the
captured identity independently of unrelated dirty notifications. Updated the
two tests (three data cases) to assert preserved Document association after an
unrelated host dataset mutation or restored manual slot assignment. Recollection
still retires the previous batch; document replacement negatives remain intact.
No production guard or native behavior changed. The original red and baseline
comparison logs are retained separately.

## Validation scope and setup receipts

- Production browser release build passed with Qt 6.11.2.
- 64 current Rust policy tests passed. The full engine Rust unit-test rebuild
  was stopped before tests ran because its separate panic/unwind configuration
  would repeat the native build; it is not claimed as run.
- Broad HTTP run: 552 passed, one 5,000-target callback timeout. The original
  runtime reproduces that same timeout in the isolated cap case. Its 10-second
  test ceiling was not raised; this extreme-load case remains unqualified.
- Ordinary quality/geometry/controls/actions/baseline: 110 passed.
- WebApp interaction: 50 passed using the maintained runner's forced-GC flag.
  An initial run omitted that flag and failed only the GC-availability assertion;
  its log is retained, not counted as final qualification.
- The historical cold-path comparator pins `30e9daf`, not this task's `066cf8b`
  baseline. Its old inspection/cache-count assertions produced four failures;
  those historical assertions were left untouched. The current ordinary script
  is byte-identical to the starting version; the maintained feed differential
  fixture is used with that actual starting source for the final comparison.
- The initial native harness build correctly rejected an old top-level engine
  archive. Any old-binary test output following that failed build is invalid for
  this change and is retained only as setup evidence. The engine archive is
  rebuilt explicitly before relinking the selected integration tests.

### Native hint journeys and current-baseline feed

The freshly linked native runner passed playback across tabs, dirty/loading
pending-action cancellation, and mixed-policy normalization. The key-routing and
family-policy process passed 96 cases. Native hint controls passed after fixing
an existing test race: back-to-back temporary configuration commands now wait
for the effective config before the next command/input. No production settings
behavior changed. Embedded resource parity is included in that passing suite.
The current-baseline feed differential passed all four cases, including fresh
pages, scrolling, changed feed content, target coverage and physical work counts.
The ordinary builder is unchanged; no policy-free speedup is claimed.

The initial broad native batch was not green: the early-input test raced the
asynchronous settings operation (fixed above), context-menu setup failed in
foreign-document cases, and retained-durable-close could not obtain a peer UI.
A separate baseline resource override reproduces the missing peer UI with the
original inventory code. These unrelated native setup gaps are not hidden or
claimed fixed by the optimization. Focused cross-origin repeats and their
baseline receipts remain alongside the full failing batch. The complete foreign
negative process also failed context-menu setup with the original runtime
(5 passed/1 failed), confirming that this gap is not unique to the optimization.

A temporary source swap used for a baseline control restored file contents but
changed its timestamp, causing an avoidable engine archive rebuild. Subsequent
controls use diagnostic resources/substitutions. The complete archive and
browser were rebuilt normally; no freshness check was bypassed or timestamp
fabricated. Unsuccessful setup runs remain separate from passing final checks.

## Full-width validation and policy availability exception

At 1280 × 720, ten valid scrolled Reddit pairs gave 486/464 ms (about 4.5%);
the top case gave 224/222 ms. These wider-page gains are smaller than the
reproduced tiled gains. No universal speedup or tail-latency claim is made.

YouTube's ordinary targets matched, but twenty pairs per position failed the
strict displayed-label fingerprint comparison. A separate diagnostic found all
hint fields equal except six labels: two configured numbered hints became
available, shifting later fallback letters. The original search-videos policy
exhausted the unchanged 100,000 predicate-step limit; the sparse inventory
completed it at 99,171 steps. The same 31 targets/actions remained available.
This is an availability improvement under the original limits, not an equivalent
label-preserving timing comparison. No matched YouTube full-width speedup is
claimed from those rejected pairs. User decision requested before accepting this
exception to the plan's identical-label requirement; currently pending.

The long-running diagnostic session ended with an empty active page/CDP timeout;
its attempted measurement is retained as setup failure, not a sample. A fresh
owned session supplied the full-width evidence. Final production smoke uses the
compiled browser with original resources (no QML stage/collector overrides).

## Production smoke and cleanup

Final compiled browser SHA-256:
`6582e4785530301549137fd1d9f772c1f149cd12421e484cb4588a3d7fdf57e5`. Original production resources
were verified active; all 80 measurements (10 per site/position/policy state)
completed without collection errors. They are smoke/reference results from
separate navigations, not matched baseline speedup evidence.

The owned browser stopped, debugging port closed and profiling socket removed;
prior desktop focus was requested. The installed executable is unchanged and
the personal Ferric profile was not used. Final data, source hashes, helper
sources, build/test logs and cleanup receipts remain in the ignored artifact
directory. No PR, push or installation was performed.
