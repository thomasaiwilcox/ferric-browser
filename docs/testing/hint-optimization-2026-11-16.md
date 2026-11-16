# Hint collection optimization — 16 November 2026

Baseline: `066cf8b`, Qt 6.11.2. Changes are measured independently and retained
only after correctness and repeated end-to-end benefit. No public interface or
policy behavior changes are intended.

Phases: classification; retained inventory; geometry; predicate reuse; native
verification. Each has a keep/revert gate: at least 5% and 10 ms median benefit
on a primary workload, reproduced in a second batch, with no material regression.
Primary cases: YouTube linux search and Reddit popular, top and 1,800 px scroll.
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

### Phase 2 — sparse retained inventory: kept

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
