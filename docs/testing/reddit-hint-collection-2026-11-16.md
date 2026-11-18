# Reddit feed Hint collection qualification — 16 November 2026

The scrolling-feed timeout fix reduces expensive offscreen candidate evaluation
and coordinates recovery with actual renderer completion. The one-second script
deadline, inspection limits and fresh activation proofs are preserved.

## Changes and checkpoints

- `c91d660`: baseline commit of the existing feature. Its complete source
  inventory matched the previously qualified installed build.
- `3dc10ea`: reject an individual target outside its document viewport before
  detailed visibility evaluation. Continue traversal through offscreen parents.
  Paired median improvement: 18–20%, with identical visible targets.
- `a7fd8ed`: defer scroll-dimension reads until a secondary scrollable target
  intersects the viewport. Reuse per-pass geometry; retain the shared scrolling
  and policy helpers' existing semantics. Additional paired improvement: 24–26%.
  Action-family dimension reads fell from 2,162 to 78–146 per fixture pass.
- `7dac20f`: snapshot outstanding dispatched scripts and notify a bounded native
  waiter when they settle. A timeout recovery waits up to one additional second,
  then captures one fresh request or ends loading. Epoch/document retirement
  invalidates recovery; late results cannot publish. This change is retained for
  correctness demonstrated with a deliberately blocked renderer.

## Controlled measurements

Qualified Qt/WebEngine 6.11.2, offscreen software fixture tests. One renderer,
serialized paired runs, alternating source order, three warmup pairs and twenty
measured pairs at each position. The fixture contains 30 posts, 540 open shadow
roots, slots, offscreen controls, sticky and overflowing descendants. Tests also
replace and append posts. Six target families preserve their visible target
identities, ordering and geometry. Five fresh-document pairs additionally check
coverage and cold collection cost.

These timings measure the ordinary collector script, excluding native policy
sealing, transport, publication and presentation. Complete Hint journeys are
measured separately below.

| Scroll position | Baseline median ms | Final median ms | Reduction | p95 ms, baseline → final |
| --- | ---: | ---: | ---: | ---: |
| 0 px | 157.50 | 91.05 | 42.2% | 224.20 → 106.00 |
| 1500 px | 158.10 | 96.65 | 38.9% | 170.10 → 102.00 |
| 6000 px | 154.35 | 93.00 | 39.7% | 167.70 → 104.90 |

Final fresh-document ordinary-collector medians: baseline
180.70 ms,
candidate 122.50 ms.
Early experiment cold-page outliers are retained in the raw records; five
samples do not establish a reliable tail-latency bound.

## Complete Hint journeys and regressions

Real Wayland `f` input, existing browser profile, publication confirmed through
public `site-policy-explain` session diagnostics. Each Reddit run has 30 entries
across three top-to-feed scrolling cycles, alternating 150 ms and 1 s pauses
after scrolling. Each entry is attempted once; failed entries remain failures.

| Run | Failed entries | Successful-entry median | Successful-entry p95 |
| --- | ---: | ---: | ---: |
| Installed baseline | 4/30 | 1447.3 ms | 2014.9 ms |
| Initial candidate, including navigation startup | 1/30 | 1060.1 ms | 1238.6 ms |
| Candidate confirmation, document already complete | 0/30 | 1020.2 ms | 1203.8 ms |

The candidate had no recorded frame timeouts in these runs. The initial
candidate's first entry failed with `interaction-collection-stale`: the page
changed during both bounded collection attempts. All subsequent 29 entries
passed. This first-load freshness refusal remains a known limitation and is
not erased by the later loaded-document confirmation. The scrolling-feed fix
is qualified; universal availability during changing page startup is not claimed.
Live content varies; the paired local fixture provides the controlled comparison.

The same candidate also passed 25 native `f` → published-session journeys on the
controlled feed: five fresh DOMs, twenty subsequent entries at multiple scroll
positions, and feed replacement/insertion. This fixture was rendered in the
owned diagnostic tab; its script/CSS were loaded by the debugging harness.

- 157 native application checks passed, including embedded resource parity,
  delayed-renderer settlement and wait bounds, Escape/navigation/reentry,
  document proofs, held gestures, blocked receivers, queries and passthrough.
- 106 QML regression checks passed: geometry (9), controls (31), actions (40),
  quality (20), and focus handoff (6).
- The final differential feed suite passed all four QtTest entries.

## Evidence and reproduction

Raw artifacts are under ignored `target/reddit-hint-fix/`: `visibility-accepted/`,
`scrollability/`, `combined/`, `build/`, `baseline-live.json`, `candidate-live.json`,
`candidate-confirmation-live.json`, and `fixture-live.json`. They include source
hashes, timings and all individual samples. Original failures remain available.
Interrupted/overlapping early trials and harness setup failures are excluded
from retention decisions. A disk-full build was retried after removing obsolete
project debug artifacts; the final build and source parity checks passed.

Reproduce the paired script measurements with the qualified SDK environment:

```sh
python3 tools/performance/check_hint_feed.py --baseline c91d660 \
  --output target/reddit-hint-feed-recheck
```

Qualified release SHA-256: `55e2a26352011842e2115849a8686f56949146f9895d5add919ef036fa98ab21`.
Installation and rollback verification are recorded in
`target/reddit-hint-fix/final-verification.json`. Saved site policies are hashed
before and after verification. No public API or configuration migration is
required. Ordinary feeds are the workload for this qualification; the deferred
5,000-target workload remains outside it.
