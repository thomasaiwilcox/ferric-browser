# S17 working plan — measured hot path and teardown

## Intent

Measure the same synthetic 500/1,000/5,000-target workload before considering any runtime change. Record raw renderer timings, Rust decode/label/ranking timings, payload handoff and overlay timing where the harness can capture them with monotonic clocks. Do not optimize unless a measured hotspot and a same-host before/after run justify it. Reuse the existing Hint registry and matcher.

## Planned test-to-path map

| Test | Production path exercised |
| --- | --- |
| T019 | Generated BrowserScripts facade, extracted PageInteractionScripts asset, resource manifest/hash parity, ApplicationWorld runner and missing/version/exception paths. |
| T020 | `hintAcquireScopeLease`, scoped descendant collection, callback ownership and `hintStopTracking` teardown. |
| T033 | Bounded `hintCollector` traversal, selector matching, visible candidate projection and truncation on the large-target fixture. |
| T043 | 10 warm-ups and 100 raw samples for 500 targets at 80/100/125% zoom and 1,000/5,000 targets; record renderer collection/fresh/preflight/action, payload handoff, Rust decode/labels/ranking and QML overlay timing with workload/build/platform identity. |
| T044 | 1,000 scope start/cancel cycles, forced GC, map/listener/observer counts, and continued use of a connected weak target. |
| T045 | Whole workspace Rust/CLI tests, `cargo xtask check`, engine WebEngine/offscreen/native-insert suite, adapter smoke, Wayland suite and generated docs/resource gates as applicable. |

## Commands to use

The S00 command ledger is the authority for available suites. This slice will use:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software target/qml-tests/qml_tests -input crates/ferric-browser-engine-qt/tests/qml-perf
CARGO_NET_OFFLINE=true QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask test engine
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
CARGO_NET_OFFLINE=true QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask check
cargo xtask test adapter
cargo xtask test wayland
cargo xtask docs
```

## Hypothesis and decision gate

Falsifiable hypothesis: if a hot-path cost dominates, its measured p95 will scale disproportionately with candidate count or exceed the 500-target/200 ms reference at the specified zoom levels. The initial workload is sequential, single-renderer-thread traversal over synthetic buttons; Rust stages operate over the returned bounded typed candidates. If timings do not identify a clear bottleneck, retain the existing implementation and report the evidence gap or qualification limit instead of adding caching or a broad refactor.

## Measurement outcome

The S01 comparison was captured on the dedicated baseline worktree with the same QtWebEngine 6.11.2, Chromium 140.0.0.0, 640x480 CSS viewport, device-pixel ratio 1 and offscreen software runner. The baseline mutation scenario was rerun without the S17-only dirty-revision assertion because that API does not exist at S01; the original full baseline run otherwise completed its common and overlay workloads before that assertion failed.

The 500-target host callback p95 changed from 74.65/64.32/68.78 ms at 80/100/125% zoom on S01 to 81.72/74.31/73.68 ms after the change. These common-case differences remain under C17's 10 ms investigation threshold. The 1,000 and 5,000 cases were retained as separate scale comparisons; 5,000-target p95 moved from 1,109.04 to 1,196.45 ms. The Rust benchmark records 100 samples after 10 warm-ups for 500, 1,000 and 5,000 typed candidates; raw stage samples are in `logs/S17-rust-performance-final.log`.

Profiling identified repeated style-chain walks, clipping geometry and per-row label membership as measurable work. The collector now holds computed-style, ancestor-visibility and clipping-rectangle caches only for one synchronous collection pass, and the overlay builds a null-prototype label lookup. A WebEngine regression changes visibility between collection calls and proves the cache does not survive the pass. With profiling enabled, the 500-target 100% p95 collector stages were 43.3 ms local visibility, 16.5 ms local hit testing and 10.6 ms clipping; at 5,000 targets local visibility was 932.6 ms and local hit testing was 640.5 ms. These instrumented values are kept separate from unprofiled baseline comparisons.

The 500-marker overlay time-to-visible p95 remains above C17's 200 ms reference ambition at all three zooms: 225.14/225.26/216.42 ms. Layout computation is the dominant measured stage at 173.83/174.02/167.23 ms p95. S17 is therefore implemented but unqualified for that overlay latency target; no claim of a 200 ms pass is made. The Qt Quick software/offscreen runner also does not qualify native Wayland rendering or input latency.

The strict workspace Clippy gate exposed existing S16 API documentation and idiom lints. Those were fixed with documentation corrections and behavior-preserving internal names/expressions; the `HintScopePurpose` and handoff-context fields are `pub(super)` implementation details, and the final engine and workspace checks passed after the edits. The nested Wayland startup smoke passed, while editable-page input qualification was not run because this Weston compositor does not support the virtual-keyboard protocol required by `wtype`.
