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

Implementation and validation pending.
