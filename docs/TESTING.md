# Testing Ferric Browser

The file chooser portal regressions use disposable D-Bus sessions with no
desktop service directories and an owned Wayland compositor. They exercise
the production request/controller code, including on-demand service activation,
late replies, cancellation, native parent identity, local URI policy, and the
required/auto fallback boundary:

```sh
cmake -S crates/ferric-browser-engine-qt/tests/portal -B target/portal-tests
cmake --build target/portal-tests --parallel 2
ctest --test-dir target/portal-tests --output-on-failure
```

These tests need Qt Wayland Compositor, KWindowSystem >= 6.28, Wayland headers
and scanner, wayland-protocols, and `dbus-run-session`. CI builds the pinned
KWindowSystem dependency in a disposable prefix using
`.github/ci/build-window-system.py`; the Arch package uses the system library.
Real desktop portal focus/consent remains a separate qualification step.

The application harness also builds `portal_browser` against the release engine
archive and its compiled QML resources. This exercises uploads, picker recovery
and cancellation in primary, secondary, private and popup windows, plus a real
download whose destination picker fails and is retried through the Downloads
panel. The journey checks the saved bytes, metadata readiness and preservation
of expanded rows after a rendered refresh:

```sh
cargo build --release -p ferric-browser-engine-qt -p ferric-browser --locked
cmake -S crates/ferric-browser-engine-qt/tests/application -B target/application-tests
cmake --build target/application-tests --parallel 2
ctest --test-dir target/application-tests -R '^portal_browser$' --output-on-failure
```

Set `FERRIC_PORTAL_TEST_ARTIFACTS` to a disposable output directory when running
that CTest entry to save the updating and ready Downloads panel renders. The
fixture's pointer and keyboard events target its own Wayland client surfaces.

`application_downloads` uses an owned loopback transfer and temporary download
directory in a private D-Bus session. It checks displayed progress, actual
Pause/Resume/Cancel buttons, one-time acceptance, native-profile isolation,
same-profile peer controls, and transport shutdown after its allocating window
retires. It also requires Downloads to open in the invoking private window,
checks private transfer cancellation and completed-file access, and holds an
actual SQLite write while cancelling a live transfer from the visible manager.
The transfer does not start until the temporary destination and picker policy
have been checked against the actual browser UI. The request tests also
exercise page-preserving direct downloads from a page or native start surface,
an actual cancelled-transfer Retry (including a fresh redirect),
private/secondary/popup owners, cross-origin redirects, profile cookies, an
absent source body and HTTP authentication. A held 1 MiB transport must continue
after its auxiliary view is released. Before-header cancellation, navigation,
owner retirement and delayed destination cancellation must release that view
without changing the source page or adopting another window.

```sh
ctest --test-dir target/application-tests -R '^application_downloads$' --output-on-failure
```

The public Downloads probe uses a short disposable browser root and retains
observations in a separate fresh artifact directory. It holds a real SQLite
writer while checking active and completed session rows and a registered action,
verifies private query opt-in and manifest policy, and requires two delivered
normal completion barriers around a private transfer on one event subscriber.
The probe additionally holds a real request before headers, rejects another
preparation with `E_BUSY`, cancels the exact operation, then completes a subsequent
request. A preparation operation completes at its native offer; separate file
bytes and a download event establish transfer completion. Source tabs and the
focused owner remain unchanged.

The native Downloads group also checks the invoking UI's script availability,
private manifest denial and a foreign profile's refusal.

```sh
python3 crates/ferric-browser-engine-qt/tests/application/run_download_ipc_probe.py --executable target/release/ferric-browser --artifacts target/download-ipc
```

Startup recovery has a separate actual-process regression for text/JSON errors,
unknown/incompatible selected roots and overlong local connection addresses.
It checks exit codes, useful next actions, path redaction and preservation of
selected/unrelated sentinels. All five XDG roots are disposable:

```sh
python3 crates/ferric-browser-engine-qt/tests/application/run_startup_recovery_probe.py --executable target/release/ferric-browser --artifacts target/startup-recovery
```

Choose a fresh artifact directory on each invocation. This probe refuses to
overwrite evidence and never resets existing browser data.

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

The repository gate includes a native download-source regression using the
version-matching Qt private SDK headers. Run it independently with:

```sh
cmake -S crates/ferric-browser-engine-qt/tests/download-source -B /tmp/ferric-download-source
cmake --build /tmp/ferric-download-source
ctest --test-dir /tmp/ferric-download-source --output-on-failure
```

It releases real binary HTTP responses after destroying the requesting view,
and verifies that both regular and private profiles reject the resulting
source-less offers safely. Live-view cases must still resolve the exact source.
All profile and XDG data belongs to a temporary process environment. The
compatibility guard is qualified for QtWebEngine 6.11.2; refresh its recorded
interface and repeat these tests before changing the SDK.
ADR-0002 documents this isolated exception to the public Qt API rule. Production
qualification retains an independent ordinary-page late native download in
`application_action_feedback`. The public Downloads probe now requires actual
TCP cancellation before response headers and a terminal operation, then checks
the next offer and full payload independently. These are separate guarantees.

`application_download_finalization` holds a real file sync using a test-only
interposer and external watchdog. It verifies input/event-loop responsiveness,
exclusive publication, collision preservation, cancellation, private completion,
owner retirement and shared-profile peer ownership. The broad Downloads run
skips those rows; the dedicated entry must run as well.

`application_downloads` also exercises the integrated explicit stream in regular
and private profiles: site service workers really control the ordinary page,
but cannot substitute its browser-owned download payload. Security-denied direct
and redirected requests never reach the file endpoint; redirect loops, HTTP
errors and truncated bodies fail cleanly. Unknown sizes stay unknown and binary
and UTF-8 response filenames retain their intended contents.

`application_download_runtime_guard` uses an owned runtime-version interposer
and disposable public CLI/IPC process. It requires a failed operation, zero
requests to the download target, unchanged source tabs and no output files.
Ordinary website activity supplies a causal barrier after rejection. Override
`FERRIC_BROWSER_EXECUTABLE` when configuring CMake to qualify a frozen browser
artifact rather than the default release path.


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

`pointerAdapterSceneMapping` exercises the production adapter with mouse-accepting
Qt Quick items. It verifies direct and ancestor translation, rotation and scale,
and window position/size changes during move, press and first release. A changed
mapping must send no initial press, preserve cleanup after an attempted press and
prevent a second gesture. A stable transformed view remains usable. These are
offscreen component regressions; DPR application cases separately cover stable
display scales, while physical monitor transitions remain manual qualification.

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

`cargo xtask test application` builds a release BrowserUi and runs the shipped
QML resources in a native Qt test harness with a disposable profile and a
loopback fixture server. It covers composed keyboard routing, local Pointer
Hint authority, scope handoffs, real modal responses, selector callbacks,
configuration persistence rejection, page API outcomes, coordinate mapping at
multiple zoom/DPR settings, and 1,000 Hint lifetime cycles with forced GC.
The accompanying CLI probe starts the release browser and checks restricted
command chains before any IPC frame is sent. This suite uses production routing
and policy; it does not substitute test implementations for them.

The offscreen clipboard case uses only the process-local clipboard. Tests on
Wayland skip that case. `cargo xtask test application-wayland` runs the same
application harness on an isolated nested Weston instance using Qt-generated
input. It qualifies that platform path, but does not prove compositor-delivered
physical input. `cargo xtask test wayland-input` requires the compositor input
facility and fails explicitly when it is unavailable. The older `test wayland`
command remains a startup smoke and can succeed without injected input.

Offscreen, Qt-generated Wayland, compositor input, authenticated live-site,
platform IME/chooser, and human/screen-reader evidence are separate gates. Record
unavailable gates as blocked rather than deriving their outcomes from startup
or synthetic fixture results.

The application suite includes production selector snapshots for inheritance,
extension, replacement and disabled families. File reload cases exercise both
immediate empty policies and asynchronous parser validation, including selector
removal, invalid CSS and stale callbacks. Reload activates reload-scoped settings;
startup/restart values remain pending. Native adapter regressions invalidate the
surface during move and press delivery and check press suppression, cleanup and
uncertain outcomes. The full offscreen application suite has a 300-second total
bound. Run timing-sensitive UI suites after compilation completes; the product's
250 ms pointer preflight deadline is unchanged.

The October 2026 hint quality repair adds T058–T061 to the keyboard interaction
matrix. The [repair record](testing/hint-quality-repair-2026-10-14.md) covers
collector admission, reserved labels, passive refresh, accessible naming, frame
geometry, marker placement, overlap selection, performance and installation.
Automated Qt/WebEngine checks and nested Wayland startup evidence are recorded
separately from physical-input and human usability qualification.
