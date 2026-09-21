# M0-162 performance measurement harness

Date: 2026-09-21  
Status: in-progress

## Observable result

`cargo run --release -p xtask -- perf --suite core` runs at least 30
monotonic-clock samples for pure-core command parsing, completion over a
synthetic 100,000-entry catalog, and switcher ranking. It emits median, p95,
p99, every raw sample, the applicable target and `within_target` result, build
profile, architecture, parallelism, and available Linux reference-environment
facts. Core is the default suite.
Large completion catalogs now use a bounded top-k selection: matching rows are
ranked once, only the best display-sized set is retained, and the final order
remains deterministic. This avoids sorting and cloning the entire catalog.
`--output PATH` writes a new JSON artifact and refuses to overwrite an existing
file.

Example:

```text
cargo run --release -p xtask -- perf --samples 30 --output /tmp/ferric-browser-perf.json
cargo run --release -p xtask -- perf --suite ui --samples 30
```

The harness deliberately labels these as pure Rust timings. Qt, WebEngine,
network, storage I/O, process-tree PSS, and native Wayland measurements remain
separate integration measurements rather than being mixed into core latency.
The `ui` suite launches the sibling release browser with a private disposable
basedir and automatic graphics selection, waits for a decoded owner-IPC reply,
samples forwarding-process IPC latency, measures main-process idle CPU over
five seconds, sums Linux `smaps_rollup` PSS/RSS across the process tree, counts
renderers, captures runtime graphics facts, requests a coordinated quit, and
removes the disposable state even after an error.

On 2026-09-20, an optimized release build completed a 30-sample run. The
100,000-entry completion workload measured median 8.730 ms, p95 10.307 ms,
and p99 11.264 ms on this host; command parsing measured 5.096 µs p95 and
switcher ranking measured 0.475 µs p95. All three workloads were within their
declared targets. The JSON artifact contained the required build metadata, raw
samples, and percentile values; it was not added to the repository.

## Runtime acceleration pass

On 2026-09-21, the Qt runtime moved from permanent 50 ms and 120 ms polling to
coalesced IPC wakeups and one-shot maintenance, focus, and scroll timers.
Blocker evidence and MPRIS state now update from change signals, tab projection
has an aligned O(n) fast path, and attached DevTools views are created lazily.
The request interceptor retains typed data internally, avoids a per-request
scratch allocation, bounds evidence in place, and coalesces UI notifications.

A release build was run on native Wayland with automatic graphics selection.
Runtime diagnostics reported the `opengl` Qt Quick backend, the `wayland`
platform, and the `i915` driver. With one blank tab and DevTools closed, the
automated UI suite reported 957 ms to owner-IPC readiness, five processes, one
renderer, 304,041 KiB PSS, and 0.20% main-process CPU over a five-second idle
interval. The PSS is about 22% below the earlier 389,341 KiB baseline that
included an eagerly created DevTools renderer. Across 30 fresh forwarding CLI
processes, IPC latency was 94 ms median and 183 ms p95, including CLI creation
and envelope validation.

The same optimized build completed a fresh 30-sample core run with command
parse p95 3.216 µs, 100,000-entry completion p95 17.077 ms, and switcher-rank
p95 0.849 µs. All declared targets passed. These are development-machine smoke
measurements under the `powersave` governor, not fixed-hardware release claims;
multi-tab, media, download, and long-running memory-plateau qualification still
remain.

ThinLTO with one codegen unit was also built in an isolated target directory
and compared over five alternating 30-sample runs. It reduced the `xtask`
binary from 39,186,184 to 18,218,800 bytes, but did not produce a consistent
latency improvement: the stable completion runs remained around 15–17 ms p95
for the normal profile and 16–17 ms p95 for the candidate, with scheduler
outliers in the normal-profile samples. Because the planned adoption threshold
was a repeatable 3% runtime improvement, the slower-build settings were not
enabled globally.

## Verification

```text
cargo test -p xtask --locked --offline
cargo clippy -p xtask --all-targets --locked --offline -- -D warnings
cargo build --release -p xtask --locked --offline
cargo run --release -p xtask -- perf --samples 30
cargo run --release -p xtask -- perf --samples 30 --output /tmp/ferric-browser-perf.json
cargo build --release -p ferric-browser --locked
cargo run --release -p xtask -- perf --suite ui --samples 30
```
