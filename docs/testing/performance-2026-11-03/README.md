# Performance investigation, 3 November 2026

Baseline: `6d02cfe642cdbdb21dd798d897643cafcaaee018`, verified against remote
`main` before work. Branch: `perf/measured-efficiency`. This register separates
historical observations, synthetic measurements and native browser qualification.

## Evidence corrections

- S25's approximately 99 ms decoding and 71 ms ranking p95 values came from
  `target/debug/deps`, as recorded in `S25.json`. They are not release timings.
- The later keyboard-interaction `completion-report.md` records an existing
  active-outline optimisation: software overlay p95 at 100% zoom went from
  226.04 ms to 183.43 ms. Neither value measures this build.
- The collector already caches styles, ancestor visibility, rectangles, frame
  mappings and rendered state within one synchronous pass.
- The 50 ms storage timer belongs to the primary window and visits durable
  window entries. Twenty windows imply roughly 400 checks/second, not 400
  independent timer wakeups.

## Findings register

| ID | Classification | Evidence and decision | Risk / remaining qualification |
| --- | --- | --- | --- |
| PERF-01 | Insufficient evidence | Repaired stale ancestor-cache instrumentation; uninstrumented callback latency is now separate from stage profiling. No collector production change. | Current GPU-backed and complex-page measurements require the qualified browser SDK. Cross-collection caching excluded. |
| PERF-02 | Insufficient evidence | Existing spatial bucketing, representative markers and active-only outlines retained. Historical optimisation acknowledged above. | Measure geometry layout, delegate creation and painting separately before changing invalidation. |
| PERF-03 | Resolved in fuzzy matching | The fuzzy matcher allocates temporary character vectors for each candidate/term. A streaming candidate is checked against the previous scoring algorithm; see comparison artifact for retention decision. Decoder unchanged. | Synthetic matcher results do not establish decoding or native keyboard latency. |
| PERF-04 | Plausible scaling risk | `browser_ui_profile_setup.rs:configure_profile` eagerly spawns seven helper threads per configured UI. No pooling or lifecycle change. | Thread scaling follows source structure; PSS, startup and first-use latency remain unmeasured. Lazy creation requires qualified lifecycle regressions. |
| PERF-05 | Resolved | One native batch formerly triggered three QML refresh handlers. Aggregate notification now triggers one; direct per-site lookup avoids building the complete map. Evidence conversion occurs after releasing the shared lock. | Native batching and QML binding tests run separately from the complete browser. Blocking decisions and property notifications are preserved. |
| PERF-06 | Confirmed redundant work | The eligible-window timer polls even without outstanding work. Inspection found polling also starts/restarts workers, flushes deferred start-page work, builds indexes and retries sources. Timer retained. | A pending-storage-only replacement would strand other work. Implement the unified scheduler only with end-to-end retry, completion and shutdown tests available. |
| PERF-07 | Insufficient evidence | Existing `completion.100k` is not the full Switcher projection path. Scalar and hint ranking have separate measurements. No projection rewrite. | End-to-end large-history queries and private-scope/pagination regressions required. |
| PERF-08 | Insufficient evidence | Aligned reconciliation fast path retained; no measured large-tab bottleneck. | Instrument synchronization counts and fallback duration on a qualified 20/100/500-tab browser. |
| PERF-09 | Insufficient evidence | Linear policy matching retained. No representative user rule-set measurement supports an index. | Any future index must preserve precedence, exception handling and attribution. |

Primary sources are under `crates/ferric-browser-engine-qt`: hint scripts and
presentation components, `src/hint_payload.rs`, `src/browser_ui_profile_setup.rs`,
`src/request_interceptor.{h,cpp}`, `src/browser_ui_library.rs` and
`src/browser_ui_query_switcher.rs`. Rust matching is in
`crates/ferric-browser-core/src/{hints,switcher}.rs`; storage scheduling is in
`crates/ferric-browser-application/src/storage.rs`.

## Measurement tooling and contracts

The `xtask perf` JSON schema is now 2. Existing metric names remain available.
Median is the midpoint for even counts; p95 uses nearest rank. p99 is null below
1,000 samples. Independent UI repetitions are in `runs`; top-level measurements
represent the first run, not an aggregate. `--samples` controls IPC samples per
UI repetition, not launch or CPU sample counts.

Process-tree CPU matches `(PID, start-time ticks)` across endpoints. New child
CPU is included; exited children make the total unavailable (`null`) because
endpoint counters cannot reconstruct their final usage. Process churn is
reported. CPU is relative to one logical processor and may exceed 100%.
Memory is an endpoint PSS/RSS sum; unavailable memory readers are counted.
Thread totals are null if any process status cannot be read; missing readers
are counted separately.
Short-lived children entirely between endpoints cannot be observed. These
metrics are appropriate for settled idle, not a substitute for task accounting
of request bursts or startup. External profilers are required for wakeups,
SQLite activity and exact active-workload process-tree accounting.

Build and run from the checkout being measured. Provenance includes invoking
checkout status and environment; it is not proof that an independently supplied
browser binary was built from that checkout. Do not mix pre-existing binaries
with a fresh harness and label them as one build.

```sh
cargo build --release -p ferric-browser -p xtask --locked
# Qt-independent ranking, including complete HintInteraction query updates:
target/release/xtask perf --suite hints --samples 100 --output /tmp/hints.json
# Repeat for pages=1,5,10,20 and presentation=internal,windows:
target/release/xtask perf --suite ui --samples 100 --repetitions 5 \
  --idle-seconds 120 --pages 20 --presentation windows --output /tmp/windows-20.json
# --page-url URL selects the same deterministic fixture in every page.
# The default is about:blank; use disposable profiles and fixture-only URLs.
# Decode requires the qualified Qt build even though its hot path is Rust:
cargo test --release -p ferric-browser-engine-qt --lib --locked \
  s17_rust_hint_pipeline_performance -- --ignored --nocapture
```

QML T043 now measures uninstrumented collection with the existing
`FERRIC_PERFORMANCE_JSON` marker and separately emits
`FERRIC_COLLECTOR_PROFILE_JSON`. The latter's timings overlap: local visibility
includes clipping and local hit testing. Instrumentation must preserve candidate
identity, order and geometry. Timing comparisons reject mixed instrumentation
modes; historical instrumented rows cannot be compared with new raw latency.

Run five alternating baseline/candidate pairs without compilation in progress.
For latency retention require at least 10% and 1 ms improvement in median or
p95, a paired-run confidence interval excluding zero, and no ordinary-workload
regression above both 5% and 1 ms. For resource changes report measured structural
reductions separately from user-visible gains. Preserve raw samples.

## Validation and environment

The host currently supplies Qt/QtWebEngine 6.12.0. Ferric's production build
explicitly pins 6.11.2 for its verified private download-source compatibility
interface. That guard is unchanged. Standalone public-Qt evidence tests can run
on 6.12.0; this does not qualify the complete browser or the new live UI workloads.

Further environment details, command outcomes and results are recorded in the
artifacts in this directory. Qualification-dependent slices above remain open;
this investigation does not claim that every planned browser benchmark ran.

## Retained changes and measured results

- `6d66449`: aggregate evidence notification, direct per-site lookup and bounded
  snapshots converted outside the lock. An isolated test requests 10,000
  notifications in 100 batches: 300 legacy property signals versus 100 aggregate
  signals, reducing expensive QML refresh invocations by **66.7%**. Existing
  property notifications remain available. Snapshot, clearing, reentrant batch
  delivery and real QML count-binding regressions pass on public Qt 6.12.0.
  This establishes removed work, not an end-to-end request latency gain.
- `78a219e`: repaired instrumentation, added the pure Rust hint-query suite,
  independent UI repetitions and configurable tab/page-window workloads,
  corrected percentiles and added process-tree CPU/thread/shutdown reporting.
  The current harness unit tests pass. Six generated collector families compile
  and stale anchors fail closed. T043's 100-candidate functional run passes in
  real WebEngine under Qt 6.12.0 software rendering, including candidate
  equivalence and activation-stage checks. Follow-ups `98d0d1d` and `737da06`
  register the native test task and strengthen measurement validity checks.
  Its timings were collected during
  other functional checks and are deliberately not used as a latency baseline.
- `523c7d2`: stream the existing bounded fuzzy subsequence scan without temporary
  character vectors. This smaller change avoids session caches and their
  invalidation/memory costs. An independent copy of the old scorer checks Unicode,
  empty terms, boundaries, gaps and the 4,096-character limit against the new
  implementation. All core hint/ranking contracts pass.

The matcher comparison uses the actual `switcher.rs` from the baseline commit
and candidate, separately compiled with `rustc -O`, without Qt. It measures
ranking all 5,000 candidate labels per sample. The source hash in
[the raw comparison](matcher-comparison.json) matches the retained source;
`candidate_commit` identifies the parent checkout and `candidate_dirty` records
that the candidate was not yet committed when sampled. Each workload has ten
warmups and 100 samples per run, over five alternating baseline/candidate pairs.
The table reports medians of the five per-run estimates. Neither binary was
measured while compilation was in progress. This is a shared development host
under the powersave governor, not fixed-hardware browser qualification.

| 5,000-candidate matcher workload | Median before | Median after | Change | p95 before | p95 after |
| --- | ---: | ---: | ---: | ---: | ---: |
| `synthetic target 42` | 7.663 ms | 2.984 ms | -61.1% | 11.305 ms | 4.696 ms |
| `st42` | 2.770 ms | 1.616 ms | -41.7% | 4.406 ms | 3.108 ms |
| `no-match-xyz` | 3.941 ms | 1.534 ms | -61.1% | 6.490 ms | 2.827 ms |
| `CAFÉ 東京` control | 1.724 ms | 1.669 ms | inconclusive | 3.276 ms | 3.120 ms |

The first three satisfy the retention threshold and paired confidence test.
The contiguous Unicode control is unchanged within measurement uncertainty.
Across all sixteen 100/500/1,000/5,000-candidate workloads, none crosses the
regression threshold. Complete hint activation, Rust payload decoding and
end-to-end Switcher latency remain unmeasured.

Reproduce the synthetic comparison from the repository root after builds finish:

```sh
python3 tools/performance/benchmark_matcher.py \
  --baseline 6d02cfe642cdbdb21dd798d897643cafcaaee018 --output /tmp/matcher.json
cargo xtask test blocking-evidence
node tools/performance/check_hint_instrumentation.cjs
python3 -m unittest discover -s tools/performance -p 'test_*.py'
```

## Qualification outcome and remaining work

The Qt-independent seven-crate test run passed **609 tests**, with zero failures
or ignored tests. The latest harness run passed 27 tests; these overlap the
seven-crate run and are not an additional independent test count. Formatting,
selected-crate Clippy with warnings denied, four measurement-contract Python
tests and the isolated evidence task pass. See [validation receipts](validation.json).

The complete release browser build fails at the unchanged SDK guard
(`6.12.0` installed, `6.11.2` required). Consequently workspace engine tests,
production UI scenarios, native GPU comparisons, storage scheduler replacement,
worker lifecycle changes and session restoration qualification remain blocked.
Only a partial old SDK is cached locally; no host packages or SDK guards were
changed.

`cargo xtask check` passes its static/lint stages but its offscreen QML suite
reports **1,221 passed and five failed** on Qt 6.12.0. Three failures also reproduce
against the untouched baseline: shared command-dialog mouse interaction,
library graph first-page/font changes, and journey destination recycling in
window mode. The download page-down and tab-mode recycling cases pass in focused
baseline reruns; a focused candidate rerun still fails download page-down and
graph layout, while the other cases pass. All five remain recorded as full-run
qualification failures. The SDK and these failures must be resolved before
claiming complete browser qualification.

Next, use a complete qualified 6.11.2 SDK and repeat the full checks and native
performance matrix. Investigate storage's complete scheduling responsibilities
before replacing its timer. Use measured 1/5/10/20-page thread/PSS results to
justify lazy helper creation. Profile current DOM collection and layout before
adding any cache or invalidation changes. Revisit PERF-07/08/09 only when their
own end-to-end measurements identify material costs.
