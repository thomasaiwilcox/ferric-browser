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

The unified keyboard-interaction matrix lives in
`docs/implementation/keyboard_interaction-v03/TEST_MATRIX.json`. Its synthetic
HTML is under `crates/ferric-browser-engine-qt/tests/fixtures/webapp-interaction/`.
`tst_webapp_interaction.qml` calls the production collector, fresh validator,
and final actions in ApplicationWorld. The native suite also checks original
Qt text entry after production shadow-root Hint focus. Baseline red logs and
qualification gaps are retained in the implementation pack's evidence ledger.

After the engine runner is built, run the opt-in renderer benchmark with:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software target/qml-tests/qml_tests -input crates/ferric-browser-engine-qt/tests/qml-perf
```

It records 10 warm-ups and 100 measured samples per scenario using the
renderer monotonic clock. JSON output includes raw collection/fresh durations,
zoom, viewport and engine user-agent. Run on an idle host and retain the full
metadata before comparing results. These component timings omit Rust
decoding/labels/ranking, payload transfer and overlay time-to-visible; they
are explicitly incomplete T043 evidence until those stages are measured.

CTest runs this suite offscreen with software rendering. Running the executable
under a Wayland compositor additionally requires a keyboard seat and an active
test window. A headless Weston instance without a keyboard seat cannot satisfy
the suite's focus prerequisite; that environment can still check installed
application startup and page loading, but cannot qualify keyboard routing.
