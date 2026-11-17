# Hint and memory optimisation implementation — 17 November 2026

Baseline: `32b0877`; installed production inputs remain `8c27c75`.

**Outcome:** no production candidate passed the agreed benefit gates. All
production changes were reverted; the measurement harness and evidence are kept.
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

### Phase 4 — attribution limit; no speculative ownership change

Existing same-source PSS/mapping evidence locates much of the difference in the
main process and anonymous/buffer mappings but does not identify a particular
retained Rust/Qt allocation. The existing owned cancellation/destruction checks
passed in the native candidate qualification. No allocation ownership defect was
confirmed, so no unrelated cache/ownership change was made.

An optional glibc `libmemusage` diagnostic crashed the disposable unmodified
baseline (PID 755092). Core review confirmed the owned temporary profile and
SIGSEGV in a QtWebEngine background worker/libc store; the main-thread stack
also contains `libmemusage`. Symbols are insufficient for an exact upstream
mechanism. Available memory and kernel logs did not indicate OOM. Profiler
interaction is an inference, not a confirmed production defect. This diagnostic
is excluded; the normal resource tests had completed without that profiler.
The extracted temporary core was removed. No personal profile was involved.

### Phase 5 — preliminary resource screens

A resource-only selector-validator candidate passed real CSS parsing, invalid
CSS, idle recreation, replacement/cancellation, timeout, load failure/recovery,
and an owned-renderer SIGKILL/recreation fixture. The latter pins and checks a
reported child renderer before signalling it; it never targets personal tabs.

Concurrent resource screens showed savings but separate-process screens were
more conservative, exposing PSS sharing and possible compiled-resource effects.
Those resource screens alone are not a shipping qualification. The candidate was therefore built normally for comparison with an isolated
baseline executable, CSS operation completion and native memory measurements.
Hint timing/control qualification would follow only if that gate passed.

### Phase 5 — rejected after compiled comparison

The normal release build passed (15m23s). The lifecycle tests passed: eight
functional cases plus fixture init/cleanup, and a separate real owned-renderer
termination/recreation case. These establish functional behaviour, not a shipping
performance result. The rejected tests and implementation remain in ignored
artifacts; baseline production sources and tests were restored afterward.

Three fresh sequential desktop pairs used separate executable copies, normal
rendering, matching native window geometry, private profiles and caches, and no
resource override or allocation profiler. Start order was reversed in pair 2.
The browser helper and qualified SDK were the same on both sides.

| State | PSS saving, pair 1 / 2 / 3, MiB |
| --- | ---: |
| Fresh one tab | 5.6 / 5.2 / 7.5 |
| Eight visited tabs | 4.6 / **−7.0** / 8.6 |
| After closing seven | 4.8 / 5.0 / 9.0 |

This does not reproduce the required 8 MiB whole-browser benefit in every
session. The optimistic concurrent resource screens were not an accurate
estimate of the compiled isolated result.

Settings latency was measured from writing the new selector configuration
through the real IPC reload operation and publication of the requested,
engine-validated value. Initial setup was corrected to wait for publication:
an operation's terminal status alone can precede that boundary. Nine reloads per
version completed successfully. Observed added times ranged from −28 to +196 ms;
five paired operations exceeded +100 ms. The roughly 45–51 ms inner parser
callback cost seen in resource diagnostics did not capture the complete workflow.

The candidate therefore failed both memory and validation-delay gates. It was
reverted. A further hint/control performance matrix and broader release
qualification were unnecessary after these failures; no claim of unchanged
candidate hint latency is made.

## Final disposition

No production optimisation passed the agreed gates. All browser production
sources match the original qualified `8c27c75` inputs. The retained changes are
an opt-in current-source feed benchmark, its alternating coordinator, and this
measurement record. The earlier recommendation to reuse native preparation was
a hypothesis; the measurements show that its removable cost is too small here.

The current evidence places the larger remaining opportunity in ordinary DOM
work, logical evidence processing and asynchronous live verification. Deleting
those facts/checks would change quality. This bounded plan ends without a new
cache, proof protocol, lifecycle mechanism or speculative ownership redesign.
The 5,000-target case and designed Insert behaviour were not changed.

The selected review runtime reuses the preserved qualified baseline after
checking 905 production input hashes against its original producer. Its binary
SHA-256 is `0f374110d84b4a6e576fe0ebf63a1fefd883b62b7378c2ed6015154f02472e38`.
Version/help, mapped native Wayland startup, healthy complete active tab and
KDE-free policy/runtime checks passed. This is reuse of a matching qualified
producer, not a claim of a new final-source compilation or full release suite.
The installed personal browser/launcher was never switched, so no installation
rollback is required. Unqualified experiment outputs must not be installed.

[Measured results](hint-optimisation-implementation-2026-11-17/results.json) and
[provenance](hint-optimisation-implementation-2026-11-17/manifest.json) retain the
receipts. Full local evidence is under ignored `target/hint-optimise-20261117/`.
The selected review launcher is `target/hint-optimise-20261117/review-build/ferric-browser`;
it contains the same implementation already installed, with no claimed speedup.

The unqualified default Cargo browser/engine outputs were removed after recording
their hashes, and the source-matching baseline test runner was restored. This
prevents accidental installation or relinking of a rejected experiment. The
selected staged baseline remains available; a future default-target build must
run Cargo normally. No archive freshness guard or file timestamp was bypassed.
