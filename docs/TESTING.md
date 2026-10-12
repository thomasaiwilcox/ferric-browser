# Testing Ferric Browser

Run the deterministic repository gate with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
```

GitHub Actions follows the `stable` channel in `rust-toolchain.toml`, including
Clippy. The cargo-deny container installs the same channel explicitly. The
workspace's declared minimum Rust version is not a separate CI toolchain pin.

Qt and QML qualification additionally uses `cargo xtask test engine`. The
deterministic checks and engine task build a Qt Quick Test runner with CMake and
run browser-chrome and WebEngine hint tests in an offscreen software-rendered
window. The runner initializes Qt WebEngine before Qt Quick Test constructs the
application, as required for shared OpenGL contexts on Qt 6.4 and later.
On Ubuntu, the runner also needs the `qml6-module-qttest` runtime module and
the base `qml6-module-qtqml`, `qml6-module-qtqml-models`, and
`qml6-module-qtqml-workerscript` imports; development libraries alone do not
install these QML modules.

Native-Wayland journeys use `cargo xtask test wayland`; they require the system
QtWebEngine stack, Weston, D-Bus, and the relevant portal services. Packaging is
checked with `cargo xtask package arch`.

Tests should assert state, effects, typed errors, and externally observable
behavior rather than private implementation calls. Schema-upgrade tests use
explicit temporary roots and must preserve an existing database on every
failure. Clean-break root tests must prove that incompatible Ferric data is
refused until an explicit reset and that unrelated legacy roots remain
untouched. Performance comparisons use release builds and record the machine,
Qt/Chromium version, workload, and sample count.

Application-boundary tests must prove that configuration updates are atomic,
profile and context mutations enter through their typed methods, and every accepted
storage ticket reaches exactly one terminal effect even when a profile closes
or is replaced. Architecture checks reject durable-resource ownership and
storage implementation types in the Qt presentation state.

The engine task also runs a native insert routing suite with Qt Quick WebEngine.
It can be run independently:

```sh
cmake -S crates/ferric-browser-engine-qt/tests/native-insert -B /tmp/ferric-native-insert
cmake --build /tmp/ferric-native-insert
ctest --test-dir /tmp/ferric-native-insert --output-on-failure
```

It uses the production key router and local HTML, requiring Qt Quick,
Qt WebEngine Quick, Qt Test and CMake. It exercises original Qt key events,
DOM keydown/keyup counts, form submit, newline, autocomplete selection,
contenteditable, input removal, modified Enter, input-method preedit, navigation,
and invalidated gestures. Lost-release regressions check that a fresh modified
Enter or IME confirmation cannot inherit an earlier ordinary Enter's mode
exit. Completion regressions move keyboard ownership to chrome or another
window before release and verify the owning session exits without redirecting
the native release. It does not qualify real websites or platform IMEs.

The suite also qualifies persistent and one-key Pass-through outside editable
fields: original keydown/keyup delivery, Escape and modified keys, held-key
repeats, overlapping keys, entry-shortcut isolation, Ctrl+Escape recovery,
navigation/epoch invalidation, lost releases, and window deactivation. A page
which leaves its shortcuts unhandled verifies that WebEngine's asynchronous
parent-item delivery cannot execute a Normal-mode browser command afterward.
Chrome tests check both window types' mode labels, recovery hints, accessible
descriptions, and existing status-bar visibility policy.

S04 adds a native-insert case that routes a retained page target through the
production `FerricPagePointerAdapter`. It records the pointer/mouse sequence,
`isTrusted`, and activation state for a local pointer-only fixture. This is
offscreen Qt event evidence; it does not qualify physical Wayland delivery or
application-level activation, and those limits remain in the slice evidence.

S19's `pointerAdapterDoubleClickSequence` fixture uses one production-adapter
request and records the native DOM sequence, details, trust flags, transient
activation, URL stability, acknowledgement count, duplicate-request rejection,
and the disabled-by-default gate. The sequence is Qt/WebEngine offscreen
evidence only. The runtime remains disabled until the required native Wayland
qualification passes; the observed trust/activation values do not qualify
permission-gated APIs. See
[`ADR-0016`](architecture/ADR-0016-qualification-gated-double-click.md).

The unified keyboard-interaction matrix lives in
`docs/implementation/keyboard_interaction-v03/TEST_MATRIX.json`. Its synthetic
HTML is under `crates/ferric-browser-engine-qt/tests/fixtures/webapp-interaction/`.
`tst_webapp_interaction.qml` calls the production collector, fresh validator,
and final actions in ApplicationWorld. The native suite also checks original
Qt text entry after production shadow-root Hint focus. Baseline red logs and
qualification gaps are retained in the implementation pack's evidence ledger.

The Site Ledger also shows an on-demand interaction snapshot: mode, focus
category, shortcut-policy source, native-prompt category, scroll-policy status,
and allowlisted counts of recent action-error categories. It never records a
keystroke stream or includes URLs, frame paths, input values, labels, or audit
identifiers. S18's example site-rule bundle is documentation data only; it is
not loaded by default and has no live-site qualification claim.

S14 extends the WebEngine interaction fixture with scoped Hint lease transfer:
the source candidate map remains available until its destination WeakRefs are
validated, then scoped collection excludes the root and every target outside
it. The fixture covers open shadow roots, same-origin frames, empty and
replaced scopes, rejected cross-origin scopes, focus/scroll independence, and
1,000 create/cancel cycles with elapsed-time output.
The stress test requires a WebEngine process started with Chromium's
`--js-flags=--expose-gc` flag so it can force collection and verify that a
connected weak target remains usable.

After the engine runner is built, run the opt-in interaction benchmark with:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software target/qml-tests/qml_tests -input crates/ferric-browser-engine-qt/tests/qml-perf
```

It records 10 warm-ups and 100 measured samples for 500 targets at 80/100/125%
zoom, 1,000/5,000 targets, a 1,000-target mutation case, and a 500-marker
overlay at each zoom. The JSON retains raw collection, fresh-target,
pointer-preflight, action, refresh, layout-compute, and time-to-visible samples
with the viewport and engine user-agent. The collector phase clock is added by
a test-only transform; it adds overhead, so use the matching unprofiled run for
baseline regression comparisons. The host callback measurement includes the
page-script return and QML callback/payload conversion, not just transport.

The separate Rust decode, label assignment, interaction setup, ranking, and
snapshot sample is opt-in:

```sh
CARGO_NET_OFFLINE=true cargo test -p ferric-browser-engine-qt --lib s17_rust_hint_pipeline_performance -- --ignored --nocapture
```

Keep the raw samples and workload/build/platform identity. The S17 evidence
records p95 as the nearest-rank 95th value (sorted sample index 94 of 100),
including full tails and the offscreen software-rendering limit. The
instrumented renderer profile, unprofiled baseline comparison, Rust stages,
and overlay visibility stages together form the T043 evidence.

CTest runs this suite offscreen with software rendering. Running the executable
under a Wayland compositor additionally requires a keyboard seat and an active
test window. A headless Weston instance without a keyboard seat cannot satisfy
the suite's focus prerequisite; that environment can still check installed
application startup and page loading, but cannot qualify keyboard routing.
