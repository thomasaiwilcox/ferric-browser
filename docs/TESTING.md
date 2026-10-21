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
and scanner, libxkbcommon headers, wayland-protocols, and `dbus-run-session`. CI builds the pinned
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

The keyboard chrome gates use the same release engine and temporary browser
roots. `application_switcher` checks completion editing, text navigation,
selection, action focus and list paging offscreen. It first compares the
compiled chrome resources, including the paging script, with the current source.
Paging waits for the complete search batch and also traverses uncached results
with background delegate incubation paused in both primary and secondary windows.
The persisted-bookmark journey also checks Tab into the selected row, Shift+Tab
back to browsing, subsequent arrow navigation, and a round trip through the
scrolling toolbar that preserves selection.
`libraryDrafts` uses 110 persisted bookmarks to check an unfinished editor across
delegate recycling and catalog pages, followed by an explicit save. Its text
entry uses Qt events; navigation and action keys use the seat in the compositor gate.
`journeyDestination` visits two loopback pages, chooses New tab or New window
through the row's controls, returns to list browsing and presses Enter. It checks
the rendered document URL and that the original page stays open. Each seed
visit must first appear in its own temporary SQLite profile. Both
destinations run offscreen and with compositor seat keys at 20-point text.
See the [journey destination qualification](testing/journey-destinations-2026-10-20.md).
`journeyGraphKeyboard` records fifteen loopback visits, opens Relationships
through its real button, pans and pages the focused map, then traverses the
relationship outline at 20-point text. It checks visible row overlap, keyboard
return to browsing, access to the search header, and Home/End plus g/G through
the same offscreen and compositor gates.
See the [library controls and graph qualification](testing/library-controls-and-journey-graph-2026-10-21.md).
`managerFormsKeyboard` creates and renames a profile, cancels a rename and saves
a named session through Return in primary and secondary windows. It checks
the actual focused recipient of each submission, held-Return suppression,
catalog publication, draft cleanup and usable focus after completion. Text
entry and explicit repeat events use Qt; navigation and fresh Return use the
owned compositor seat in the Wayland gate. The resource guard also compares
the profile/session managers and shared feedback component.
See the [manager keyboard qualification](testing/manager-keyboard-forms-2026-10-21.md).
`favouriteEditorKeyboard` adds, edits and opens a favourite in primary,
secondary and private windows. It verifies address-to-name Return, held-key
suppression, request identities, newer text and a real SQLite rejection/retry.
An owned writer lock orders an older metadata read before save completion;
that read must leave the editor pending. This ordering step invokes the focused
field's acceptance in the same event-loop turn. Other fresh Return/navigation
events use the compositor seat in the Wayland gate. The resource guard includes
the start page, both button components and the focus-revealing scroll column.
See the [favourite editor qualification](testing/favourite-editor-2026-10-21.md).
The same native journey also reuses the saved address in another slot, then
chooses the bookmark through its keyboard picker and changes its name again.
It requires the submitted name in both the tile and bookmark, the same bookmark
ID, one bookmark record and one occupied slot in primary, secondary and private
windows. Storage checks additionally exercise duplicate addresses and roll back
the name and both affected slots when an insert trigger rejects the pin.
See the [favourite naming qualification](testing/favourite-names-2026-10-21.md).
`favouriteRequestsKeyboard` holds a real edit save behind an owned SQLite
writer in regular windows, delivers bookmark captures through the production
engine-action route, and requires the original text, selection and identity
to survive. It also reviews waiting captures in order, explicitly discards a
newer local edit, and changes a selected bookmark's address through the backend.
A direct save using the old visible address must fail without renaming that
bookmark; a keyboard choice of the updated address must then save correctly.
Primary, secondary and private windows use the same journey, with synchronous
private saves. The resource guard additionally compares the runtime controller.
See the [favourite request qualification](testing/favourite-requests-2026-10-21.md).
`downloadPaging` reads thirty durable download records and owned files in
primary and secondary windows at 20-point text. It checks page overlap,
selected-row actions, full details, toolbar traversal and reopening through
both offscreen and compositor seat keys. The resource guard includes the
download manager and workspace. QML checks also cover disclosure recycling,
completion/retry/header focus recovery, and 320-pixel/26-point controls.
See the [Downloads keyboard qualification](testing/downloads-keyboard-2026-10-21.md).
`settingsDrafts` exercises filtering, persisted numeric changes, canonical
value acknowledgment, held-Return suppression, toolbar refresh, discard and
reopening in primary and secondary windows. Its navigation and fresh Return
use the owned compositor seat in the Wayland gate; text and explicit repeat
events use Qt. The resource guard includes Settings, its row editor and its
workspace. The QML suite also covers reordered rows, native undo/selection,
owner handoff, rejected controls and 320-pixel/26-point draft actions.
See the [Settings editing qualification](testing/settings-drafts-2026-10-21.md).
The same journey also blocks the owned profile's runtime override path with
a directory, submits through Return, reads the actual writer's failure details,
then repairs the path and retries the preserved draft. It checks receipt IDs,
effective values, persisted output and synchronization of saved settings
between regular profiles sharing the same setup. The QML
checks cover pending progress, synchronous and asynchronous results, reopening,
scope changes, Reset and edits made while a previous request is still pending.
`durablePersistenceFailurePreservesConfiguration` and
`selectorValidationRejectsWholeCandidate` additionally exercise writer and
WebEngine selector validation receipts through the production backend.
See the [Settings save qualification](testing/settings-save-feedback-2026-10-21.md).
`application_settings_durability` preloads a test-only sync interposer into its
owned browser process. After a normal saved setting, it fails directory sync
for exactly that temporary profile's state directory. It checks that the next
value is applied and written, and that Settings shows the crash-durability
warning and its detail. Other paths and file syncs pass through unchanged.
The journey fixtures also require each first real visit from a native blank
page to persist under its actual document URL. The graph fixture rejects a
persisted engine placeholder and reports load-event URLs and owned database
rows on failure. See the [first-navigation qualification](testing/first-navigation-2026-10-21.md).
Internal QML reference/type errors and binding-loop warnings fail this suite. The
`application_keyboard_chrome_wayland` gate delivers paging, Tab/Shift+Tab,
completion cycling, Ctrl+P and Ctrl+Space through the owned compositor's seat;
it requires one non-repeated native press/release pair, expected modifiers,
and cleared modifiers afterward. Its single keyboard seat uses a US keymap.
The separate `application_command_editing_wayland` gate sends Qt-generated
key/mouse events to Wayland windows to check completion insertion, caret,
undo and explicit preedit events. These gates do not qualify physical hardware,
a full desktop compositor, GPU rendering or platform IMEs.

```sh
ctest --test-dir target/application-tests --parallel 1 -R '^application_(switcher|keyboard_chrome_wayland|command_editing_wayland)$' --output-on-failure
```

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
