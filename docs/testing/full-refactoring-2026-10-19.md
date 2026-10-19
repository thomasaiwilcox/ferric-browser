# Application boundary refactoring qualification

Branch: `refactor/application-boundaries`, based on `0035783`.
Date: 2026-10-19. Host: Intel Core i5-7200U (two cores, four logical CPUs),
Linux, rustc 1.98.0, Qt 6.11.2. Native application tests use the offscreen
platform, software Qt Quick backend and Chromium `--disable-gpu`.

## Reproduction

Development checks use a separate target directory with debug information
disabled; release benchmark settings remain those in the workspace manifest.

```sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export CARGO_TARGET_DIR="$PWD/target/refactor"
cargo xtask check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check --manifest-path fuzz/Cargo.toml --bins --locked --offline
```

`xtask check` includes architecture and authority guards, QML lint and interaction
tests, native profile-lifetime and download-source tests, generated documentation,
release inputs, license policy, formatting and locked workspace tests.

## Results

Results are summarized in
[the implementation record](../implementation/full-refactoring.md).

- Final `xtask check`: pass, including 1,116 Rust tests and 631 QML cases.
  The two opt-in ignored Rust tests cover hint performance and label ergonomics.
- All 12 fuzz binaries: compile with the locked offline dependency graph and
  complete 1,000 uninstrumented smoke runs each.
- Strict workspace Clippy with all targets and warnings denied: pass.
- Native application coverage: all 130 methods exercised; 389 distinct cases
  pass across serial runs and dedicated environments, including setup/cleanup.
  A fresh release-linked monolithic run passes 379 cases with ten expected
  skips in 974.57 seconds under the corrected 1,200-second aggregate limit.
- Native context reload: 33 pass.
- Download runtime compatibility and isolated native notifications: pass.
- Both 1,000-cycle lifetime checks and private Wayland clipboard transport: pass.
- Dedicated download finalization: heartbeat harness corrected after release
  diagnosis; ten consecutive passing runs and an expected failing stall control.
- IPC probes for terminal state/provenance, selection lifecycle, window routing,
  Switcher privacy and downloads: pass.

The broad native application replay executed all 130 discovered test methods in
two isolated processes. The initial groups reported 126 passes / 5 failures /
9 skips and 243 passes / 7 failures / 1 skip, including data rows and per-process
setup/cleanup. Failed methods were replayed after correcting asynchronous
publication assumptions, obsolete diagnostic expectations and the identified
interaction defects. These totals are not an aggregate count of unique tests.

The final focused application replay passes 24 cases without failures or skips.
It includes every initially failed application method, the captured-request and
startup-input regressions, regular/private/temporary window settings,
configuration feedback and revocation of a held pointer confirmation after a
relevant configuration change. Context reload passes all 33 cases.

The original CTest outer watchdogs (application 480 seconds, context reload
90 seconds, downloads 180 seconds) expired during concurrent compilation. The
direct native replays use longer outer watchdogs while retaining their original
per-assertion deadlines. Passing a replay does not qualify those shorter CTest
watchdogs on this host.

Dedicated download finalization passed through its owned file-sync interposer
and independent watchdog. The ordinary download run intentionally skips these
seven rows because they require that harness.

The ordinary download replay reported 63 passes, one failure and seven expected
skips during compilation. Its regular redirect-loop row observed three server
requests where it expects two. Both regular and private redirect-loop rows pass
unchanged in a separate run after builds stop (four passes including setup and
cleanup). The original full-download CTest watchdog is not claimed green.

The isolated native-notification gate and download-runtime compatibility guard
also pass through their dedicated CTest runners.

## Follow-up serial qualification

All native test binaries were relinked against the final development library
before this follow-up. Tests ran serially, without concurrent task builds, using
the original CTest timeouts and assertion deadlines:

```sh
ctest --test-dir target/refactor-application --verbose --parallel 1 \
  -R '^application_(downloads|context_reload|offscreen)$'
```

| Gate | Result | Time |
| --- | --- | ---: |
| Downloads | 64 pass, seven expected finalization skips | 146.76 s / 180 s |
| Context reload | 33 pass, no failures or skips | 60.06 s / 90 s |
| Full application | Timeout after 198 passes, nine skips, no assertion failures | 480.04 s / 480 s |

Both redirect-loop rows pass in the complete download suite; the earlier extra
request did not recur. The full application's aggregate timeout still occurs
on this host without compilation running. It is not a passing CTest gate.

The interrupted file-chooser method and every subsequent application method
were then replayed in two serial groups with 480-second outer limits. The groups
report 117 passes / no skips / no failures in 348.73 seconds and 68 passes /
one opt-in skip / no failures in 214.97 seconds. All 15 file-chooser rows pass.

Coverage was reconciled against the executable's `-functions` inventory:
all 130 methods were exercised, with 379 distinct passing case names including
setup/cleanup, ten distinct environment/opt-in skips and no assertion failures.
Repeated setup/cleanup and the partly executed file-chooser method are counted
once in that distinct-case total.

These executables use unoptimized Rust development code. The repository's
`prepare_application_tests` runner normally builds with `--release`, so this
development timeout alone did not justify changing the release budget. The
subsequent release-linked application run and measured budget correction are
recorded below.

### Lifetime, service and Wayland follow-up

The remaining dedicated gates ran serially after the application replay:

| Check | Result |
| --- | --- |
| Download runtime compatibility | Pass, 2.25 s |
| Isolated native notifications | Nine cases pass, 19.41 s |
| 1,000 hint lifetime cycles | Pass, 69.19 s |
| 1,000 cross-origin frame lifetime cycles | Pass, 295.77 s |
| Private Wayland clipboard transport | Three cases pass, no skips, 2.96 s |

The hint loop retains exactly 8,765 Qt objects before and after; its final retained
elements, observers and windows are empty, with scope/tracking cleared. The frame
loop checks lease count zero after every cycle and after final collection.

The Wayland clipboard test uses a private nested Weston compositor and its
existing parent-window ownership guard for injected input. The runner now accepts
`--executable` so this test can select the current native binary explicitly:

```sh
python crates/ferric-browser-engine-qt/tests/application/run_clipboard_wayland.py \
  --executable target/refactor-application/application_tests
```

This qualifies compositor-delivered clipboard gestures, not physical keyboard
input. After adding these dedicated environments, reconciliation finds 389
distinct passing application case names including setup/cleanup, all 130 test
methods exercised, and no initially skipped application case left without a
later passing execution. The monolithic development-build gate still timed out.

The earlier download-finalization harness exposed an intermittent failure. Its first
follow-up run reports eight passes and one failure: the `retire` row's 10 ms QML
heartbeat timer records fewer than five ticks during a 120 ms wait
(`tst_downloads.cpp`, heartbeat assertion). Every other row passes. Three
subsequent unchanged runs each pass all nine cases, in 14.19, 14.43 and 13.86
seconds. Thus the failure occurs once in four runs during this follow-up. This
is evidence of an intermittent timing failure, not proof that it is harmless or
that the production UI is blocked by file sync. The release-linked investigation and correction are recorded below.

### Finalization heartbeat investigation and fix

A fresh standard release build reproduced the original heartbeat failure in one
of six diagnostic runs. The failing retirement row recorded four QML timer
callbacks but six native GUI-thread timer callbacks in the same 120 ms window.
[Qt documents that QML timers follow the animation clock](https://doc.qt.io/qt-6/qml-qtqml-timer.html);
they are not an independent measure of GUI event dispatch.

Replacing only the timer was insufficient: the ninth repetition with a native
timer still recorded only two callbacks in 169 ms. The fixed test therefore
requires five native GUI-thread callbacks within the same one-second response
budget already used for command input and owner retirement. It checks elapsed
wall time as well as callback count, and verifies that neither the barrier
release nor the worker-exit marker exists afterward. The worker-thread check,
external ten-second watchdog, command responsiveness, cancellation, ownership
and final file assertions remain intact. This tests bounded GUI progress while
sync is blocked; it does not certify a 120 ms rendering or input-latency budget.

A temporary negative control queued a 1.1-second sleep on the GUI thread in the
retirement row. The revised assertion failed with one callback after 1,100 ms;
the other eight cases passed. The injected stall was removed and the fixture
rebuilt before normal validation. All ten subsequent CTest repetitions passed
(nine cases each, no failures or skips; 126.10 seconds total), with no concurrent
builds. Log: `/tmp/ferric-refactor-fixed-release-heartbeat.log`. No production
download code changed.
Diagnostic logs: `/tmp/ferric-refactor-heartbeat-diagnostic-{0..5}.log`,
`/tmp/ferric-refactor-heartbeat-fixed.log` (the insufficient timer-only fix), and
`/tmp/ferric-refactor-heartbeat-negative.log` (expected negative-control failure).

### Release application timeout correction

The standard release build (`cargo build --release --locked -p
ferric-browser-engine-qt -p ferric-browser`) completed successfully in 12m 27s,
using the manifest's optimized profile with debug information. All native
application targets were rebuilt against that current release static library.
No compilation ran alongside timing-sensitive tests.

For diagnosis, only the generated `application_offscreen` outer limit was
extended from 480 to 1,200 seconds. The release suite continued making progress
past eight minutes and completed in **974.57 seconds: 379 passed, zero failed,
ten expected environment/opt-in skips**. The complete single-process run includes
all 130 methods. Its skipped notification, lifetime and Wayland cases remain
covered by the earlier dedicated development-linked runs, not by this release
run. The 480-second budget was therefore too small even for an optimized build.

`tests/application/CMakeLists.txt` now gives this aggregate suite 1,200 seconds.
Per-assertion deadlines and the budgets of all other CTest gates are unchanged.
Reconfiguring CMake preserves the same tested command, environment and timeout.
This is an aggregate test-budget correction, not a production performance fix.

```sh
ctest --test-dir target/application-tests --verbose --parallel 1 \
  --repeat until-fail:10 -R '^application_download_finalization$'
ctest --test-dir target/application-tests --verbose --parallel 1 \
  -R '^application_offscreen$'
```

Logs: `/tmp/ferric-refactor-fixed-release-heartbeat.log` and
`/tmp/ferric-refactor-fixed-release-application.log`; result manifest:
`/tmp/ferric-refactor-fixed-release-results.json`.

### Focused dispatch review corrections

The review identified two behavior regressions in shared generic dispatch.
Registry mode validation is restored at the generic `spawn` entry point, before
process launch or userscript lookup; separate native action eligibility remains
unchanged. `scroll-target select` again returns success immediately and discards
the chain suffix while interactive selection owns input. Noninteractive policies
continue their chain, and invalid arguments still reject it.

New release-linked native regressions reproduced the defects before the fix:
eight disallowed modes incorrectly accepted `spawn`, and the interactive
scroll-target chain continued (nine failures; eight passes including setup and
cleanup). After the fix, the expanded action-feedback CTest gate passes all
**40 cases with zero failures or skips in 58.46 seconds**. It checks all ten
modes, userscript rejection before lookup, five scroll-target policies/invalid
inputs, and explicitly exercises native external selection actions from Insert
mode. The regressions are included in both the full application fixture and the
existing CI action-feedback gate.

The standard optimized release build passes. Baseline log:
`/tmp/ferric-review-dispatch-before.log`; final action-feedback log:
`/tmp/ferric-review-release-dispatch.log`. The native fixtures were relinked to
the corrected library before testing. Generated installer logs and Python
caches are now ignored without deleting local files.

### Remaining release matrix and Switcher fixture correction

After the dispatch fixes, all native targets were relinked against the corrected
release library. The 22 remaining CTest gates ran serially, excluding the already
qualified full application, finalization and newly passing action-feedback gates.
The initial pass completed 21 gates successfully and failed one Switcher case
(1,432.69 seconds total); this initial matrix invocation is not claimed green.

`invokingWindowOwnsTheCombinedLimit` failed to apply its temporary limit after
rapidly opening ten tabs. It passed in isolation but failed again in the complete
Switcher suite. Added diagnostics showed mode `command`, status `Journey write
queued`, and limit 100: the fixture had not completed the setting submission.
The fixture now waits for the final tab's owned native view, valid frame,
completed load and `rb://blank` URL before sending the next command-line gesture.
Tab-count publication alone did not establish that native activation was ready.
The expected limit, keyboard submission and privacy/ownership assertions remain
unchanged. All three complete Switcher repetitions then passed: 11 cases each,
no failures or skips, 51.05 seconds total. No production Switcher code changed.

Selected current-release results:

| Gate | Result | Time |
| --- | --- | ---: |
| Action feedback, including dispatch regressions | 40 pass, no skips | 58.46 s |
| Switcher after fixture readiness correction | 11 pass × three runs | 51.05 s total |
| Downloads | 64 pass, seven dedicated-harness skips | 138.12 s |
| Context reload | 33 pass, no skips | 51.29 s |
| Hint lifetime, 1,000 cycles | Pass, Qt object count 8,765 → 8,765 | 65.95 s |
| Cross-origin frame lifetime, 1,000 cycles | Pass | 311.23 s |
| Clipboard | Pass | 131.28 s |
| Browser surface suite | 190 pass, no skips | 511.32 s |
| Deferred requests and all five portal gates | Pass | See gate manifest |

Close requests, file-chooser backend/privacy, notification consent/transport,
isolated native notifications, runtime compatibility and both high-DPI gates
also pass. Public IPC and window-routing probes pass against the corrected
release browser (6.13 and 5.31 seconds). Private nested Wayland clipboard testing
passes against the corrected native binary (3.25 seconds).

Every registered CTest gate now has passing release evidence: 23 gates from this
review pass, plus the earlier full-application and finalization qualifications.
Those two earlier gates were not repeated after the localized dispatch changes;
the new behavior regressions and affected native actions were tested in the
current 40-case gate. This is qualification across recorded runs, not one fresh
all-green invocation of the entire matrix.

Logs: `/tmp/ferric-review-release-remaining.log`,
`/tmp/ferric-review-switcher-diagnostic.log`,
`/tmp/ferric-review-switcher-repeat.log` (pre-correction failure), and
`/tmp/ferric-review-switcher-fixed.log` (three passing runs). Gate timings are in
`/tmp/ferric-review-release-gates.json`; IPC/Wayland results are in
`/tmp/ferric-review-release-probes.json`.

### Bounded fuzz smoke

All 12 fuzz binaries were built with the locked offline dependency graph and
executed sequentially, 1,000 runs each, with no crashes or timeouts. Targets:
`action_target`, `bindings`, `command_parser`, `contexts_routes`, `hints`,
`ipc_frames`, `session_snapshot`, `site_rule`, `state_sequences`,
`switcher_matching`, `url_and_clean_link`, and `userscript_results`.

Each target used its checked-in corpus copied into a disposable directory, so
the repository's seed files were not mutated. Flags were `-runs=1000`,
`-seed=20261019`, `-max_len=16384`, `-timeout=10` and `-rss_limit_mb=1024`, with a
60-second process limit per target. All logs report completion of 1,000 runs.

These stable-toolchain binaries lack coverage and sanitizer instrumentation;
libFuzzer reports missing sanitizer hooks and lack of interesting coverage.
This is corpus/mutation crash smoke, not a coverage-guided campaign or sanitizer
qualification. `cargo-fuzz` and a nightly toolchain are not installed here.

## Limits

All registered native CTest gates now have passing release evidence across the
recorded runs, with the version and replay distinctions noted above. Physical
keyboard input, hardware rendering, real desktop notification delivery and
coverage-guided/sanitizer fuzzing remain unqualified. The two ignored Rust tests
also do not establish hint performance or label-ergonomics qualification.

Persisted configuration, database and session formats are unchanged. Protocol 3
is intentionally incompatible with protocol 2; external clients must update.

## Performance method

The opt-in `ferric-browser-runtime/examples/effect_dispatch.rs` harness creates
100, 1,000 and 5,000 tabs, performs 100 warm-up reload dispatches, then records
three samples of 1,000 dispatches each. Tab creation is outside the timed loop.
The same harness was built in release mode against baseline `0035783` and this
branch. The baseline copy differs only in formatting.

Only serial samples taken after builds and native tests stop are retained as
qualification evidence. This measures core/runtime reload dispatch, including
invariant checks and observation production. It does not measure page loading,
Qt rendering, end-to-end input latency or browser startup.

| Tabs | Baseline median | Refactored median | Baseline / refactored |
| --- | ---: | ---: | ---: |
| 100 | 8.394 µs | 4.313 µs | 1.9× |
| 1,000 | 406.713 µs | 61.262 µs | 6.6× |
| 5,000 | 7.848 ms | 0.396 ms | 19.8× |

Raw samples: [baseline](full-refactoring-dispatch-baseline.csv) and
[refactored](full-refactoring-dispatch-after.csv). The baseline ran first, then
the refactored binary; neither overlapped a task build or native test. The host
retained its existing `powersave` CPU governor. These are three samples on one
host, without a statistical claim about other workloads or machines.

Measured executable SHA-256 values (the refactored implementation is the working
tree, with no implementation commit created):

```text
baseline 3b93f554c75c4446f7a3a98ef06bfc0fa57765e7434cd57876d8b1480ffaab1f
after    a4dc83318c7d8366a440bea2cfeb88e03c9dcf135dbd62a36d400c745cdaeac1
```
