# S01 baseline reproduction and fixture scope

The detached worktree `/tmp/ferric-keyboard-v03-pinned` is at the exact pinned
commit `4337056f632de0c710943fa6df76c67cf14b8b47`. Only synthetic tests/fixtures
were added there. No production repair was applied before the red run. The
current checkout's Hint code matches the pinned implementation; its script
resource also contains later accepted history helpers.

## Intended patch and exercised production paths

Add self-contained HTML under `tests/fixtures/webapp-interaction/` and a new
test in the existing QML runner. T002 exercises `hintCollector -> hintFresh ->
hintClick/hintFocus` in ApplicationWorld and inspects an exact effect counter
or deep active element. T003 covers assigned slots and inert ancestry. T004
covers frame-border projection, document replacement and adoption. T005
covers visibility, disabled/transparent ancestry, overlays, detach and exact
replacement. T006 preserves the viewport overflow regression. T008 observes
actual focus and synchronous handler redirection.

The native T002 fixture uses those same production script builders, evaluates
them in ApplicationWorld, and then sends original Qt text input through
`FerricBrowserKeyRouter`. Failure occurs at final focus before text entry at
baseline. This is native-qt-offscreen evidence, not a physical Wayland test.

The benchmark is opt-in in `tests/qml-perf/`, outside the ordinary functional
test directory. It measures production collection and one fresh validation
using renderer `performance.now()`, 10 warm-ups and 100 samples per scenario.
It preserves raw timings and viewport, zoom, Qt/user-agent metadata.

## Observed baseline results

| Assertion | Result |
| --- | --- |
| T002 open-root final click | Reproduced: discovery/fresh succeed; final click returns false. |
| T002 open-root final focus | Reproduced in QML and native: discovery/fresh succeed; final focus returns false. |
| T003 slotted control with newly inert shadow ancestor | Did not reproduce a defect in this case; rejection already passes. |
| T004 frame-border projection | Reproduced: x is 50 instead of 57 CSS pixels; 7px border omitted. |
| T004 adopted retained element | Reproduced: old frame candidate remains fresh after adoption into top document. |
| T004 replaced frame document | Existing stale rejection passes. |
| T005 disabled fieldset descendant | Reproduced: collector includes a disabled native button. |
| T005 transparent ancestor | Reproduced: collector includes an ordinary button beneath opacity zero. |
| T005 hidden/opacity-zero/disabled/inert/detached/covered/replaced selected button | All seven negative cases already pass. |
| T006 zero-height body viewport propagation and partial clipping | Passes at baseline. |
| T008 ordinary focus | Passes at baseline. |
| T008 synchronous focus redirection | Reproduced: final action returns true while the other input is active. |

QML totals: 13 passed, 7 failed. Native focused totals: 2 passed, 1 failed.
These are actual failing positive assertions, not passing implementation
evidence. Fixes belong in S02. The retained red logs include exact assertion
locations and Qt version.

An extended baseline run (`logs/S01/pinned-extended-red.log`) retains those
assertions and adds the S02 adversaries before production changes. It records
15 passed and 11 failed. The four additional failures are assigned-slot
clipping, excessive composed ancestry, a same-URL covering sibling admitted
by fresh validation, and a fixed synthetic password value used as a label.
Literal HTML-like label text and post-action map/listener/observer cleanup
already pass. `logs/S01/fixture-sha256.json` identifies the exact fixture/test
bytes used in the extended run; earlier red logs refer to their earlier test
line numbers and are retained separately.

## Commands actually executed

From the main checkout, with the newly added tests in the detached pinned worktree:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software target/qml-tests/qml_tests -input /tmp/ferric-keyboard-v03-pinned/crates/ferric-browser-engine-qt/tests/qml/tst_webapp_interaction.qml
cmake -S /tmp/ferric-keyboard-v03-pinned/crates/ferric-browser-engine-qt/tests/native-insert -B /tmp/ferric-keyboard-v03-native-pinned
cmake --build /tmp/ferric-keyboard-v03-native-pinned --parallel 2
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu /tmp/ferric-keyboard-v03-native-pinned/native_insert shadowHintFocusAcceptsOriginalText
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software target/qml-tests/qml_tests -input /tmp/ferric-keyboard-v03-pinned/crates/ferric-browser-engine-qt/tests/qml-perf
```

## Measurement limitations

All six exploratory scenarios finished with 100 measured samples each: 500
targets at zoom 0.8/1/1.25, 1000/5000 targets at zoom 1, and a 1000-target
mutation case. These ran concurrently with baseline Rust/C++ compilation.
Their collection p95 values were respectively 152.4, 147.7, 395.4, 456.7,
2089.4 and 370.8 ms. They cannot be treated as a stable comparable performance
baseline. The nearest-rank p95 formula and all raw values are in
`logs/S01/pinned-performance.json`. Chromium's user-agent reports version
140.0.0.0; the installed public-API probe recorded in S00 reports
140.0.7339.225. The probe is an installed library version check, not a live-site
compatibility or security-support claim.

Collection timing combines traversal, selector matching, geometry and hit
checks. Rust payload decoding/labels/ranking and QML overlay time-to-visible
are not yet measured separately. Transfer/preflight/native delivery are also
not qualified by this harness. T043 therefore remains incomplete, and no
optimisation or threshold pass is claimed. Extend the stage instrumentation
and rerun the same host/workload at idle before making a comparison.

## Provenance, review and handoff

All HTML is authored synthetic test data under the repository's GPL-3.0-or-later
licence; no third-party framework assets or live network dependencies are used.
Counters are DOM data attributes shared across script worlds. They contain no
account identifiers, typed private data or secrets. The live task matrix is
`S01-live-task-matrix.json`; all live journeys are explicitly not_run.

Self-review only. Fixtures retain the original element identity and assert
negative effects, not just candidate counts. They do not inject fake keyboard
events. No observed result is described as universal site compatibility.
S02 must preserve the already passing cases while repairing the seven proved
failures. Deeper-slot/clipping/depth, same-URL sibling, privacy payload and
callback lifetime cases are still required in S02; this baseline harness
does not claim exhaustive coverage of their broader contract.

## Current-checkout integration

The S01 harness was committed as `af154a2` without production edits. Running
the full current QML directory records 95 passed and 11 failed: all 80 prior
cases stay green and the eleven new failures match the pinned run. The full
native CTest run records 49 passed and one new shadow-focus failure. Configure
and build both succeed. Exact command/result records are in
`logs/S01/current-results.json`.

Self-review strengthened frame replacement readiness: wait for a marker in
the new document instead of text shared with the old document. The focused
replacement case passes on both current and pinned production source, in
`current-frame-replacement.log` and `pinned-frame-replacement.log`. This is a
test synchronisation repair, not a new production defect.

S01 remains `implemented_unqualified`: the harness and initial sample set
exist, while positive activation and full T043 qualification do not pass.
S02 owns repair of the proved target defects. No dependent native capability
is enabled by this handoff. The narrow post-action teardown case is passing,
but it does not prove the complete T020 callback/lease contract; that remains
open for S02 and later scope handoff work.
