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

`application_repeat` checks rejected commands, partial chains, canceled clean-link
previews, counted keyboard navigation, scrolling, zoom and new-window replay in
the production Qt adapter. `application_repeat_cli` exercises rejection and
recording through a real disposable browser process. The open-target gates also
check `.` and macro playback after clean-link and context decisions, including
background tabs and changing contexts before replay:

```sh
ctest --test-dir target/application-tests --output-on-failure \
  -R '^application_(repeat|repeat_cli|open_targets|open_targets_wayland)$'
```

`application_context_selection_cli` checks browser-created contexts through
real CLI opens and three owned browser processes. Its 19 scenarios cover live
selection, saved membership, cold launches with user and built-in configuration,
profile affinity, removed authored definitions and corrupt generated catalogs.
Both created context IDs must survive restarts. See
[the context selection evidence](testing/context-selection-2026-10-23.md).

`application_cli_output` checks closed pipes and genuine stdout failures through
the public CLI. Its 32 process cases include a reader that closes early, closed
stderr, `/dev/full`, and accepted commands in an isolated live browser. The
fixture uses disposable XDG/data directories and a stub for desktop-status output.
It never changes the desktop's default browser. See
[the CLI output evidence](testing/cli-output-2026-10-23.md).

`application_open_targets`, `application_open_targets_wayland` and
`application_open_targets_cli` check destination routing through the running
browser. Explicit `tab` opens preserve the original document, `tab-bg` keeps the
selected tab, and `current` remains the default. Native cases include clean-link
confirmation/cancellation and accepted or dismissed context prompts in primary
and secondary windows. The public CLI fixture also covers profile/context
selectors and multiple URL inputs. See
[open destination routing](testing/open-target-routing-2026-10-23.md) for evidence.

`application_tab_recovery` and `application_tab_recovery_wayland` exercise `u`
and Ctrl+Shift+T in primary, secondary, private, temporary and popup windows.
Their 40 keyboard cases check original tab position, pin/mute/zoom, neighbors,
fresh renderer identity and page focus. Pinned cases use an explicit targeted
close. Two additional cases consume the live undo record and reload a saved
checkpoint through the storage worker, covering current and version-2 files.
They also select the recovered pinned neighbor and require its saved page.
Focused QML checks cover row identity and lazy loading when pin grouping
reorders replacement or appended session plans.
The CI browser-surface selection includes both entries. See
[the closed-tab recovery evidence](testing/closed-tab-recovery-2026-10-23.md).

`application_last_tab_undo` and `application_last_tab_undo_wayland` cover Undo
after closing the sole tab. Their 39 scenarios use `u` and Ctrl+Shift+T across
primary, secondary, private, temporary and popup hosts. They require reuse of
the exact automatic fallback, restored pin/mute/zoom, a focused web document
and removal of the automatic start-page history entry. Incoming pages and
drafts, navigation back to start, explicitly opened native tabs, changed tab
settings and unfinished favourite edits must survive. A further close/undo
sequence checks descriptor order and one-time fallback reuse. Favourite edits
use an independently delivered command because the editor owns ordinary keys.
The Wayland gate observes owned-seat delivery. Both gates are in the CI filter.

`application_session_recovery` and `application_session_recovery_wayland`
exercise the recovery dialog with a corrupt checkpoint, no checkpoint, and an
owned storage read held behind a test interposer. Eight scenarios cover retry,
Escape, `n`, Alt+D, repeated restore keys, document identity and draft retention,
late completion rejection, subsequent named-session preview, and page focus.
The Wayland entry delivers keys through the owned seat. Both entries are in
the browser CI selection. These isolate the real dialog and worker after
bootstrap; they do not qualify process-crash startup or merging recovery with
existing live tabs. See [the recovery dialog evidence](testing/recovery-dialog-2026-10-23.md).

`application_startup_recovery` and `application_startup_recovery_wayland`
exercise an untouched initial start page, an incoming link, multiple incoming
tabs with closed work, and a new tab arriving during a held recovery read.
They reopen the actual profile before recovery, check current renderer and form
draft identity, pin/mute/zoom, merged Undo, and a save made before a restored tab
is visited. `application_startup_recovery_restart` launches real browser
processes on an owned bus and loopback server, kills them after durable saves,
and recovers twice through native input. It verifies live CLI-created tabs,
exact saved addresses, lazy HTTP request counts and duplicate-free second
recovery. Its input plugin is offscreen-only and never shipped with Ferric.
All three entries are in the CI browser selection. See
[the startup recovery evidence](testing/startup-recovery-2026-10-23.md).

The keyboard chrome gates use the same release engine and temporary browser
roots. `application_navigation_recovery` uses an owned loopback server that
first drops requests and then serves a recovered document. It checks keyboard
reopening, exact addresses with hidden query parameters, same-address Open,
editing and submission of an address containing command separators, focus
handoff, Back and window isolation in primary, secondary, private and popup
windows. After a failed POST, reopening must produce GET without another POST.
Opening a completed POST's address without its fragment must also produce GET.
Fragment visits on completed GET and POST documents must preserve the document
and its method, settle the loading state, and preserve Back history.
Reloading a completed or failed POST must select Keep page initially: Enter cancels
without another request, and `y` confirms exactly one POST with the captured
cache option. The same journey checks these decisions in every window type.
Cancelling a failed-form reload preserves its failed state and restores the
error dialog only if it had been visible before the reload decision.
`application_navigation_recovery_wayland` repeats the recovery journey using
compositor seat keys and checks native press/release delivery. The fixtures
compare the compiled recovery, window and navigation resources with source
before running. QML checks additionally cover stale tabs/views/addresses, safe
initial Enter, unavailable Back and 320-pixel windows at 26-point text.
`application_form_reload` and `application_form_reload_wayland` hold an owned
POST response open and exercise pending, streaming and stopped responses,
completion during confirmation, and Stay on unfinished or completed pages,
including immediate Reload retry and cancellation of Open address and Back in
primary, secondary, private and popup windows: 44 data rows per platform.
The setup waits for completed startup and form visits to enter Ferric's history;
Back cases require an available previous entry before invoking the command.
A callback from the response proves the streaming document has run
before Reload. The server counts requests and records their bodies. Keep page
must send no POST and retain the document and its current loading state;
approval must send exactly one additional POST with the original body and
requested cache option. A response may finish while confirmation remains open.
Before response headers, Reload must reopen the previous GET without another
POST. Rejecting BeforeUnload must allow another browser Reload decision.
Stay must retain the document and page scrolling without a failure dialog.
The unfinished response settles as cancelled and a completed page remains
complete. QML checks retain real DNS, network, TLS, HTTP, download and error-page
failures, expire cancellation records with their owner, and consume them once.
Both entries compare the compiled form, navigation, request-controller and window
resources with source; the Wayland entry uses compositor seat keys.
`application_stop_state` and `application_stop_state_wayland` send the configured
`s` binding in all four hosts across seven phases: complete, failed, pending
GET/POST, streaming GET/POST and queued Open. Native start notifications must
reach retained tab state; reselecting a loading tab must retain provisional
feedback. Stop must preserve settled states, stop active loads, keep the captured
target and page focus, and avoid resubmitting a request. The fixture compares
retained state, visible state, actual engine loading events and document markers.
The CI browser-surface selection includes both Stop gates.
See [the Stop evidence](testing/stop-loading-state-2026-10-23.md).
`application_streaming_history` and `application_streaming_history_wayland`
hold an owned GET or POST response open, prove its document has run, then press
Shift+H and require the immediately preceding page. Their 64 rows per platform
also cover `2H`, a native start-page anchor, returning to an earlier document at
the same URL across an intermediate visit, two completed visits at the same
address with a response nonce proving the returned document, and requests
still awaiting headers. Initial completed visits must be recorded before each
journey, and no response tail is sent before Back. Every window
type uses its own view and UI; the fixtures compare their compiled navigation
resources with source before running. QML checks cover delayed owner expiry,
signed queued counts and unavailable proof. Recording a streaming visit must
leave its loading and form state unchanged.
The same journeys query Journey before and after traversal and compare the
actual visit ID, including the earlier of two visits to the same URL and
an address containing encoded spaces and Unicode. Two queued Back commands
followed by a fresh Open and another Back must still select the original
Journey node. These native fixtures qualify the memory-backed state. QML
regressions exercise both native URL/index update orders and distinct visits
at the same URL, preserving the selected visit while those facts settle. Load
success before a native model update must retain the resolved destination
position, including stale logical anchors and redirects. Stop and BeforeUnload
Stay must release that position. Counted Forward checks assert that the numeric
prefix was accepted and include the final history snapshot on failure.
Follow-up commands while a preceding Back's model is stale must stay queued:
count two must resolve offset two after the model moves. Signed counts combine,
and an expired owner prevents execution.

`application_journey_history` uses the production executable, public CLI,
isolated profiles and a loopback server. Tab polling uses public `tabs.query`
IPC to avoid starting a process for every observation. It inspects the durable
SQLite `journey_current` row in three cases: returning from an unfinished stream to an
exact completed visit; returning to an unrecorded stream whose second response
completes, requiring exactly one new durable node; and returning to the older
of two same-URL visits while their writes await an owned SQLite writer lock.
The last case waits for the returned document's title before releasing the
lock. With ordered application admission, a later saved background visit provides
a FIFO storage barrier before pointer assertions. A deterministic application
test queues two visits before selecting the older pointer and checks the actual
SQLite result, preventing new requests from passing older queued writes.
The database reader caches the Journey database and opens
it read-only. Each run keeps observations and browser logs beneath
`target/application-tests/journey-history-artifacts`. All three durable cases
pass for v6. The 64-case native matrices
pass on both platforms, but an earlier offscreen `private-post-pending` run
retained a cancelled UI state after reaching the completed destination. This
intermittent observation remains unresolved; subsequent diagnostic runs and
their limits are recorded in
[the dated evidence](testing/navigation-recovery-2026-10-22.md#v6-compiled-verification).

```sh
ctest --test-dir target/application-tests -R '^application_streaming_history(_wayland)?$' --output-on-failure
ctest --test-dir target/application-tests -R '^application_journey_history$' --output-on-failure
```

To stress the pending-header case in one owned private window, the native
fixture accepts `FERRIC_HISTORY_PENDING_REPEATS` (1–32, default 1). Other phases
always run once. The fixture buffers native loading metadata and prints it
when its completion check observes a cancelled state; the completion assertion
and Journey identity check remain required.

```sh
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME= \
QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu FERRIC_HISTORY_PENDING_REPEATS=32 \
target/application-tests/application_tests streamingHistoryTraversal:private-post-pending
```

`application_keyboard_guide` and `application_keyboard_guide_wayland` verify
the Guide's window and UI owner in primary, secondary, private and popup
windows. They cover command-bar entry, the default `g ?` shortcut, search,
Escape, page focus after closing, independent queries in overlapping windows,
and a new `binding-explain` query after editing and clearing the old search.
Six QML cases open or reopen a compact article and immediately request Page
Down, Space or `j` without a render barrier, then reverse the movement.
The native journey retains failure-only focus and scroll-geometry facts;
source/resource checks include the Guide and focus controller. See the
[first-scroll evidence](testing/guide-first-scroll-2026-10-23.md).
The Guide journey also checks that `zoom` selects Page zoom, compact `gg`
finds the top/bottom feature, reader and list hints follow real key events,
and a configured shortcut retains the feature's other bindings. `/` from the
list and reader returns to search with the query selected, preserving webpage
scroll, browser mode and page-search text. The seat uses the physical `AB10`
key for this gesture. QML checks
cover selection on query changes versus refreshes, focused search/copy/control
hints, Escape in the section chooser, and compact hints at 26-point text.
The Wayland entry sends compositor seat keys and checks their native identity.
Both gates reject stale embedded Guide and window resources.
`application_privacy_identity` and `application_privacy_identity_wayland` check
regular/private/temporary identity in primary, secondary and popup windows.
They compare labels with actual engine profile persistence, cover hidden chrome,
all three status policies, command/search entry and cancellation, all tab edges,
large fonts and popup title changes, and require stable page geometry and focus.
The shared seat driver preserves the real title binding and requires a unique
mapped title/PID match. QML adds 320-pixel header checks at 26 and 60 points.
See the [privacy identity evidence](testing/privacy-window-identity-2026-10-23.md).

`application_live_titles` and `application_live_titles_wayland` exercise a
settled page changing its title in primary, secondary, private and popup windows.
They seed a cached search, verify the new title in Switcher and `yank title`,
change matching titles with the Switcher open, and require unchanged query,
focus, URL, document target and history position/count. They also cover bounded
Unicode/control filtering, background tab updates, retained titles after tab
selection, rejected stale callbacks, and a live detach without document reload.
See the [live title evidence](testing/live-page-titles-2026-10-23.md).

```sh
ctest --test-dir target/application-tests -R '^application_live_titles(_wayland)?$' --output-on-failure
```

`application_switcher_navigation` and `application_switcher_navigation_wayland`
cover full navigation and `history.pushState` with the Switcher already open in
primary, secondary, private and popup hosts. A fixed initial title prevents
title-change notifications from masking URL invalidation. Repeated backend
queries and visible rows must reflect the destination, preserve the query and
focus, carry the current generation and activate through Enter. Non-popup cases
also navigate a background tab without changing the selected page. See the
[navigation freshness evidence](testing/switcher-navigation-freshness-2026-10-23.md).

```sh
ctest --test-dir target/application-tests -R '^application_switcher_navigation(_wayland)?$' --output-on-failure
```

`application_switcher_live_updates` and `application_switcher_live_updates_wayland`
edit a search while real pages change their titles every 10 ms. Primary,
secondary, private and popup scenarios require results and further label updates
before that activity stops, stable query/action focus, Escape returning to the
page, and unchanged document targets. An eight-window scenario finds and updates
a result in the last window while the invoker stays busy. Focused QML checks use
twelve paced sources, including slow first results, and cover query replacement
and closing with work pending. See the
[live-update evidence](testing/switcher-live-updates-2026-10-23.md).

```sh
ctest --test-dir target/application-tests -R '^application_switcher_live_updates(_wayland)?$' --output-on-failure
```

`application_page_search` and `application_page_search_wayland` cover native
matching, result retention and activation. The shared search-feedback matrix
checks all three status policies in primary, secondary, private and popup
windows: native match position/total, n/N and wrapping, no matches, page focus
after acceptance/cancellation, and unchanged webpage viewport dimensions.
It also checks a narrow window with 26-point configuration, including the
feedback and previous/next symbols using their window's effective font.
The preedit matrix uses real `QInputMethodEvent` events and requires Return and
Escape to leave the search surface in charge until composition finishes; a
fresh action after commit must work. The Wayland gate sends fresh search and
activation gestures through its owned compositor seat. Preedit and held-repeat
probes use explicit Qt events.
QML adds keypad Enter and 320-pixel feedback/font checks. These checks do not
qualify a platform IME. See the [search usability evidence](testing/page-search-usability-2026-10-23.md).
The direction matrix starts searches with both `/` and `?` in all four hosts.
It checks repeated n/N steps and wrapping, counted explicit/default commands,
forward actions, Guide command equivalents, preserved page focus, and a fresh
query in the opposite direction. Previous/next button clicks must preserve the
query direction and return focus to the editor. The Wayland matrix uses owned
seat keyboard events and explicit Qt pointer events for those button clicks.
See the [direction evidence](testing/page-search-direction-2026-10-23.md).

`application_switcher` checks completion editing, text navigation,
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
`commandHistoryEditing`, included in both Wayland gates and the offscreen
switcher suite, has 24 cases across primary, secondary and private windows.
Each types a draft and recalls two real profile-local commands. Undo must step
through each recall to the draft and its earlier typed edit; Redo and Down must
restore the expected commands and draft without execution or focus loss. The
matrix also checks Up/Down after Undo, fully restoring the draft, editing a
recalled line, Escape followed by fresh command entry, and Undo after Down
returned to an empty or non-empty draft. Two Rust policy
checks cover exact recorded text, Unicode, unavailable positions, new edits and
a draft equal to a history value. Six QML cases
also cover empty drafts, Unicode, selected text and unchanged history boundaries.
See the [command-history editing evidence](testing/command-history-editing-2026-10-22.md).
The expanded compositor-seat gate has a 180-second process budget and a
210-second CTest budget; all delivery, focus and completion assertions remain.
`tabKeyboardContinuity` checks command and Alt+1 tab selection, Switcher
selection, closing the active first or last tab, cancellation after a tab changes behind
Switcher, and command/search cancellation in primary, secondary and private
windows. Each case checks the actual document, native page focus, Tab reaching
a real page button, and `j` scrolling. The fixture establishes initial page
focus once and never repairs focus after the operation under test.
`application_tab_focus_wayland` runs the same matrix with compositor seat keys
in a separate gate. See the [tab focus evidence](testing/tab-keyboard-focus-2026-10-23.md).
`commandModifiedArrows` compares Shift+Up/Down with a plain Qt TextField in the
same primary, secondary or private window. The actual command editor must keep
the same text, cursor and selection. It runs in both Wayland gates and the full
offscreen switcher suite. `commandCompletionPreedit` now checks ten gestures
against real `QInputMethodEvent` preedit: Right, Space, Up/Down, Tab/Backtab,
Ctrl+N/P, Escape and Return. No history, completion, submission or dismissal
request may escape during composition; normal shortcuts must resume afterward.
The QML runner also injects real input-method events into the source component,
commits Japanese text and verifies a fresh Return submits that exact text.
These are explicit preedit-event checks, not platform-IME qualification. The
full offscreen switcher gate has a 360-second budget for the expanded matrix.

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
