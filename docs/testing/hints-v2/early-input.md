# Input during initial hint collection

Hint input buffering preserves ordinary hint keys typed
before the first Rust hint session exists. Previously, the keyboard router
consumed these keys but Rust returned `no-session`, so fast label entry and
Enter could disappear.

The workspace retains at most 256 input operations for its current view and
replays them through the existing Rust interaction API after successful live
collection. It interprets slash and space at replay time, preserving text-mode
edits in order. Activation or cancellation ends replay and discards the remaining
operations, including a second Enter. Explicit queued input takes precedence over
the initial automatic-follow response.

Mode changes, view changes, tracking cancellation, target-search transitions,
native-window deactivation, modal input blocking and shutdown discard the queue.
Replay also requires the current collection epoch, tracked view and host keyboard
gate. Overflow closes hints with a notice instead of applying a partial input
sequence. Rust still assigns labels, chooses the target and validates selection
and actions against the live page.

Refreshes of an existing session continue to process keys immediately. Target
search uses its existing query path. Pointer hints can retain label edits, but an
early Enter is never synthesized into a later pointer-confirmation gesture; a new
Enter press and release are required after collection.

The native `hintEarlyInput` test drives the production window keyboard router
with synchronous press/release events, explicitly confirms Rust has no session,
and checks both primary and secondary windows. Cases cover label-and-Enter input
before collection starts and while collection is pending, text edits, Escape,
navigation, tab/view changes, native focus loss, shutdown, automatic follow,
pointer confirmation, overflow and input during refresh. Tests deliberately hold
the initial collection gate for the native-focus case so renderer speed cannot
decide whether cancellation is exercised.

Pointer cases enter through the existing local UI command, then send their early
label and Enter keys through the window router. The existing keyboard-entry and
held-confirmation regression remains in the same suite. The first full run had
49 passes and two failures before the new pointer fixture entered Hint mode; the
local UI entry removes that fixture's dependency on binding setup.

Reproduction uses the qualified Qt 6.11.2 environment through
`tools/performance/with_qt611.sh` (including its configured SDK/runtime paths).
Configure the native application harness under
`../hints-direct-pr-build/application` with `FERRIC_ENGINE_LIBRARY` and
`FERRIC_ENGINE_CXX_INCLUDE_DIR` pointing to the selected Cargo engine archive and
its generated `cxxqtbuild/include` directory, as required by the existing harness.

```sh
tools/performance/with_qt611.sh cargo build --release --offline --locked -p ferric-browser -p ferric-browser-engine-qt
tools/performance/with_qt611.sh cmake --build ../hints-direct-pr-build/application --target application_tests -j2
tools/performance/with_qt611.sh ctest --test-dir ../hints-direct-pr-build/application -R '^application_hint_controls$' -V
python3 tools/performance/check_hint_cold_path.py --input crates/ferric-browser-engine-qt/tests/qml/tst_hint_presentation.qml
python3 tools/performance/check_hint_cold_path.py --input crates/ferric-browser-engine-qt/tests/qml/tst_hint_outline.qml
```

These are offscreen/software functional correctness checks, with no native
latency or energy claim. Source review found no remaining issues after adding
the shared host keyboard gate.

Original integration results on 8 November 2026, before extracting this focused
branch from the earlier implementation workspace:

- Release browser and engine library: passed (18m 55s); native harness: built.
- Full `application_hint_controls` suite: 51 QtTest results passed, zero failures
  or skips (144.93s). This includes all 26 new primary/secondary cases and existing
  native controls, navigation, scoped hints, target search and refresh regressions.
- Embedded resource parity: all 124 bundled QML/JavaScript source files and
  manifest hashes match the working source.
- Hint presentation: 12 QtTest results passed; hint outline: three passed.
- `git diff --check`: passed. Full Rust unit tests were not run for this QML change.

Local logs: `/tmp/ferric-early-input-build.log`,
`/tmp/ferric-early-input-native-build.log`,
`/tmp/ferric-early-input-native-relink.log`,
`/tmp/ferric-early-input-regressions-final.log`,
`/tmp/ferric-early-input-presentation.log` and
`/tmp/ferric-early-input-outline.log`. The first fixture failures remain in
`/tmp/ferric-early-input-regressions.log`; the corrected pointer-only run is in
`/tmp/ferric-early-input-pointer.log`.

The focused branch keeps the same input implementation and session-ID guard.
It excludes the unused benchmark observers, instrumentation, controller and
implementation-pack history.

Focused branch validation on 8 November 2026:

- 125 QML results passed: ten collector differential/lifetime/read-count results
  and 115 action, control, geometry, quality, presentation and outline results.
- `cargo fmt --all -- --check`, the KDE-free dependency policy and the six-family
  instrumentation compilation/stale-anchor check passed.
- All four measured collector/test source hashes still match
  `cold-path-results.json`. The input workspace matches the installed, tested
  implementation after removing unused benchmark observers.
- The fresh optimized release browser and engine build passed (24m 13s).
- The fresh `application_hint_controls` run passed all 51 QtTest results, with
  zero failures or skips (211.54s), including all 26 new cases and byte/hash
  parity for all 124 embedded source files.
- The native functional harness used `-O0 -g0 -DNDEBUG` against the optimized
  release engine. No native timing measurements were collected.

Fresh branch logs are `/tmp/ferric-pr-release-build.log`,
`/tmp/ferric-pr-native-build.log`, `/tmp/ferric-pr-native-regressions.log`,
`/tmp/ferric-pr-qml-functional.log` and
`/tmp/ferric-pr-cold-functional-final.log`.
