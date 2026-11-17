# Hint and memory optimisation implementation — 17 November 2026

Baseline: `32b0877`; installed production inputs remain `8c27c75`.
Apply the extreme Rust mechanical-sympathy skill to the measured whole path.

## Ordered work and acceptance

1. Establish a current-source synchronized-frame benchmark and preserve baseline artifacts.
2. Prepare immutable native verification inputs once per captured request; keep all live checks.
3. Reuse the original parsed catalogue after an exactly equal fresh recapture.
4. Profile accepted code; permit one additional local conversion/buffer candidate only for a measured 20 ms hotspot.
5. Attribute main-process allocations; fix confirmed unintended retention only.
6. Evaluate a temporary, generation-safe WebEngine selector validator with one-second idle teardown.

Each latency candidate must reproduce both 5% and 10 ms first-label median benefit
in two independent batches of twenty alternating pairs after five warmups, with
identical target/action projections. Controls allow no repeated regression above
max(5%, 10 ms). At most one extra inconclusive batch. Revert rejected production
changes and retain evidence. Memory changes require at least 8 MiB whole-tree PSS
savings in each of three native paired sessions. Selector validation may add less
than 100 ms; ordinary hints must retain their responsiveness.

Preserve eligibility, offscreen logical facts, labels, all freshness boundaries,
work limits, cancellation, trusted input and designed Insert behaviour. No new
public API/schema, progressive labels, website exceptions or 5,000-target work.
Use Qt 6.11.2, serial performance/build work and at most two build jobs. Commit
accepted phases separately. Run targeted negatives per phase and combined native,
resource and KDE-free checks for a retained final build. Personal installation
is separate from producing a reviewable test build and rollback receipt.

## Execution log

- Starting tree clean. Existing work committed before implementation.
- Added an opt-in current-source feed benchmark using the existing synchronized
  Qt render observer; baseline engine and browser preserved under ignored
  `target/hint-optimise-20261117/` before production changes.

### Phase 1 — rejected

Fresh release build passed (15m03s). Ten focused native proof/cancellation/work
cases passed (12 including fixture init/cleanup). Two batches of 20 alternating
pairs for each primary case retained exact ordered projections and viewport.

| Case | Batch 1 baseline/candidate ms | Batch 2 baseline/candidate ms |
| --- | ---: | ---: |
| Collection top | 278.94 / 278.53 | 277.53 / 277.65 |
| Collection scrolled | 328.49 / 318.97 | 276.96 / 271.82 |
| Combined top | 345.29 / 350.87 | 286.56 / 284.36 |
| Combined scrolled | 281.30 / 283.99 | 282.28 / 280.33 |

No case met both 5% and 10 ms in both batches. Reverted the native candidate;
its patch and build receipt remain in ignored evidence. No control matrix was
needed after failing the primary gate. Within-batch pairs are authoritative;
load variation between batches is visible and not hidden.

### Phase 2 — stopped at measured cost gate

A separate Qt 6.11.2 native diagnostic replayed the actual 300-card evidence:
92,552 annotation characters and a 3,460-character fragment catalogue. Across
ten blocks of 200 operations, annotation/document conversion and parsing cost
about 1.81–1.86 ms per operation; catalogue conversion/parsing cost 0.074–0.086 ms.
This is diagnostic execution cost, not first-label performance. Avoiding the
second catalogue parse cannot plausibly meet a 10 ms user-visible gate on this
workload. No production implementation was retained for this follow-up.

### Phase 3 — no qualifying local conversion candidate

The accepted production path is unchanged. The measured native conversions are
below the plan's 20 ms hotspot threshold. Existing renderer profiling locates
substantial cost in discovery, geometry and logical evidence traversal, rather
than the permitted local native materialisation changes. No additional cache,
wire-format change or traversal redesign was introduced.
