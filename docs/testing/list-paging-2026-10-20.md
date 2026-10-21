# Paging through browser lists — 2026-10-20

The ongoing keyboard usability audit found that Page Up/Down skipped unseen
results in narrow windows and at large font sizes. The switcher moved a fixed
eight results; History and Bookmarks estimated row counts using 54 pixels per
row. Real rows vary with titles, action buttons, font size, and expanded details.

## Reproduced failures

The original QML implementation failed all six wide/narrow/short viewport cases
across the switcher and library. For example, a switcher page showing results
0–1 jumped to results 7–8. A library page showing 0–2 jumped to 8–10.
Page Down also left a tall expanded row instead of reading its remaining text.

Application regressions reproduced skipped results in the primary and secondary
switchers and in a bookmarks panel backed by a disposable SQLite catalog.
The bookmark fixture waits for the real metadata refresh before opening the
panel; pending reads are not treated as an empty catalog.

An additional regression caught Page Up stopping below the controls above the
first library row. The list's focus revealer could pull the viewport back to
the selected delegate while the user was paging through its header.

## Repaired behavior

Both surfaces use rendered row heights and viewport height when selecting a
page. The helper measures only the rows along that page, materializing a nearby
delegate when needed. Keyboard focus stays with the invoking search field or
library list. Active kinetic scrolling stops when keyboard paging takes over.

Rows taller than the viewport scroll with overlap before navigation leaves
them. Page Up reaches the complete header above the first library row; Page
Down reads that header before entering the results. Library paging keeps focus
on the list so automatic delegate-focus scrolling cannot undo these positions.
Tab then enters the selected row's actions, including after reading expanded
details, rather than jumping to another cached row's controls.

## Verification

`tst_list_paging.qml` checks complete forward/backward traversal without unseen
row gaps, useful page distance, resizing, empty lists, focus retention, expanded
details and a header taller than the viewport. It covers 12, 20 and 26 pt fonts.
The existing switcher and library suites remain part of the regression run.

`switcherPaging` exercises the compiled production resources in primary and
secondary windows. `libraryPaging` uses 30 real persisted bookmarks. Both check
rendered page boundaries after actual Qt key events and preserve keyboard focus.

```sh
cargo build --release -p ferric-browser-engine-qt -p ferric-browser --locked
cmake --build target/application-tests --target switcher_tests --parallel 2
ctest --test-dir target/application-tests -R '^application_switcher$' --output-on-failure
```

Qt/WebEngine 6.11.2 with offscreen software rendering is the automated test
environment. Physical Wayland input, authenticated live-site browsing and human
accessibility qualification remain separate parts of the overall audit.

Final results:

- Release browser and engine build passed.
- Complete QML suite: 754 passed, zero failures or skips.
- Application switcher suite: 18 passed, zero failures or skips; CTest passed
  in 41.60 seconds, including the primary/secondary and persisted-library
  paging journeys.
- Focused paging suite: 16 passed; existing switcher and library suites passed
  all 22 and 18 checks respectively.
- `cargo fmt --all -- --check`, `git diff --check`, and QML lint's
  critical/error/fatal diagnostic gate passed.
- Narrow 20 pt switcher and library renders were inspected after paging.

The first broad QML run was externally terminated by the tool host before
completion. The final complete run above passed after the release build; the
terminated run is not counted as a successful gate.


Follow-up during the command-completion audit: native paging produced intermittent
no-progress observations in the secondary window. The fixture's font setup and
frame measurements were tightened, and its final full run passed, but an earlier
focused no-progress result remains unexplained. This is still an open
investigation; the passing runs above do not establish that every timing case is
fixed. See [the follow-up record](command-completion-editing-2026-10-20.md).


## Selected result actions and Wayland delivery follow-up

Five subsequent secondary-window paging runs completed without reproducing the
intermittent no-progress observation. An initial owned-compositor run also
passed complete Page Up/Down traversal in both windows and the persisted
bookmarks panel. This is additional evidence, not an established cause or fix
for the earlier intermittent result.

The next keyboard journey exposed a separate deterministic problem: after
selecting a switcher result with arrows or Page Down, Tab could enter a cached
earlier delegate and change the selection. A narrow 20 pt QML fixture moved
from result 7 to result 4 after arrows, and from result 3 to result 2 after
paging. Production-resource application cases reproduced result 7 changing to
result 4 in the primary window and result 5 in the secondary window.

Tab now enters the selected result's first action. Search status controls
remain before the result actions when present. Shift+Tab from the first action
returns to the preceding search control; other action buttons retain native
forward/backward traversal. The compact footer and accessible description
explain the action gesture. QML checks cover arrows, pages, the last result,
details-only status, refresh-only status, and both status controls, including
backward return and activation of the selected result's action.

`application_keyboard_chrome_wayland` uses the owned compositor's keyboard seat
instead of Qt Test key injection for paging, action focus, completion cycling,
Ctrl+P pinning and Ctrl+Space. Each gesture must arrive at the browser as a
press/release pair with a nonzero native scan code and the expected modifiers.
The browser chooses only its own mapped surface by process ID and unique title.
The fixture has disposable XDG roots and a private D-Bus session with no desktop
service activation. This qualifies compositor-to-client keyboard transport in
the software-rendered fixture; it does not qualify physical hardware, a full
desktop compositor, GPU rendering or platform input methods.

The application suite also compares five chrome QML resources and the paging
script with the current source before running cases. This prevents an old compiled QML resource
from silently qualifying a changed source file.

```sh
ctest --test-dir target/application-tests -R '^application_(switcher|keyboard_chrome_wayland|command_editing_wayland)$' --output-on-failure
```


## Dynamic row layout and estimated extents

The compositor-delivered run subsequently reproduced a real Page Down failure
inside a tall bookmark row. With the selected row at y=9774 and height 354,
the 253-pixel viewport should have advanced within that row. Instead, clamping
against the estimated list length moved the viewport backwards to y=9099,
showing rows 24–25 while selection remained on row 27. Waiting for polish after
the failure left the same wrong position, so a later test measurement was not
the explanation.

The trace also showed newly created neighboring rows at provisional height 204
before their complete layout reached 354. A separate rendered QML regression
made Page Up skip row 26 when leaving a tall row 27 in all three narrow sizes.
Two bounds regressions independently reproduced invalid clamping of valid
positions inside a measured row. All five cases failed the original helper.

The helper now finishes each measured row's visible child layouts before using
its height. Its scroll bounds include the selected row's actual top and bottom,
so an estimate cannot send the reader away from valid text within that row.
The focused suite has 30 cases including these regressions. A source-level
preflight passed the complete ten-check compositor journey before the final
release-resource rebuild; that preflight is not counted as shipped-resource
qualification.

The first modified-key fixture attempt delivered no modifiers. A full-key-event
attempt then lost Shift on the first gesture in newly created windows. Inspection
of [Qt's keyboard source](https://github.com/qt/qtwayland/blob/v6.11.2/src/compositor/compositor_api/qwaylandkeyboard.cpp#L474)
indicates stale repair-cache state across focus changes. The fixture now sends
and clears explicit `wl_keyboard` modifier packets, with masks
derived through libxkbcommon from its controlled US keymap. It also sends the
modifier key presses/releases. Client receipts require one non-repeated main
press/release pair, expected modifier flags and no held modifiers afterward.
Qt documents the modifier limitation of its simpler seat helper in
[QWaylandSeat](https://doc.qt.io/qt-6/qwaylandseat.html#sendKeyEvent).

The isolated secondary-window no-progress observation needed a further
investigation, separate from these two demonstrated geometry defects.

## Complete-batch traversal follow-up

An in-memory trace reproduced a delivered Page Down that correctly advanced
inside a selected row while the viewport was 127 pixels tall. The temporary
search-status panel then disappeared, expanding the viewport to 261 pixels.
Containing the now fully visible row returned the viewport to its earlier
position. Comparing only the selection and scroll offset across that change
incorrectly classified the key as dropped. Results can be current and populated
while another window's source batch is still being searched.

The fixed-layout application traversal now waits for all source-batch entries
and pending source reads to finish. Its progress, row-gap and keyboard-focus
assertions remain enabled for every key; it does not retry navigation keys.
The geometry probe also reads delegates through native Qt APIs, avoiding a new
root-context property that could re-evaluate unrelated QML bindings.

`switcherDeferredPaging` additionally evicts the initial page, disables caching
and pauses background delegate incubation before traversing all 100 commands in
both directions, in both windows. This checks uncached delegates without
assuming that each page must change selection: tall rows must remain readable.
It runs both offscreen and through the compositor keyboard gate. Temporary
script interceptors and trace code are removed before final qualification.

## Final release-resource qualification

The release browser and engine build passed on Qt/WebEngine 6.11.2. The complete
QML suite passed **785 checks**, with no failures or skipped cases; the focused
paging suite contributes 30 checks. The final application gates ran serially
after the native test executable finished linking:

| Gate | Qt Test checks | Result |
| --- | ---: | --- |
| `application_switcher` | 30 | Passed |
| `application_command_editing_wayland` | 12 | Passed |
| `application_keyboard_chrome_wayland` | 12 | Passed |

All **54 native checks** passed, with no failures or skipped cases, in 170.71
seconds overall. Each gate passed its compiled-resource comparison before its
journeys. The compositor gate includes complete ordinary and uncached forward
and backward traversal in both windows, real persisted bookmarks, selected-row
action focus, completion model refresh and configured shortcuts. Internal QML
reference/type errors and binding loops remained fatal to the tests.

QML lint reported no critical/error/fatal diagnostics. `cargo fmt --check`,
Python compilation of the fixture runner and `git diff --check` also passed.
This remains software-rendered automated qualification; physical hardware,
desktop compositor integration, platform IMEs and human accessibility evaluation
remain outside these gates.

The subsequent [library actions and background-search follow-up](library-keyboard-actions-2026-10-20.md)
found that another source batch could begin after the initial readiness check.
It replaces the transient loading panel with search-field progress when current
rows remain usable, and finishes pending ancestor layouts before paging. That
record contains the newer 802-QML/58-native qualification and preserves the
interim failure evidence.


## Ubuntu CI font follow-up — 2026-10-21

PR #16's Ubuntu run failed four paging checks that passed with the local
JetBrains Mono font. Using Ubuntu's DejaVu fonts reproduced the failures.
Page Up could skip row 26 after leaving an expanded row 27: measuring an
uncached candidate moved the viewport, even when that candidate was rejected.
Containing the selected row afterward left the viewport too far up the list.

The helper now polishes the final selection and explicitly anchors the completed
page at the selected row's bottom for Page Down, or its top for Page Up. The
intervening rows stay visible despite revisions to ListView's estimated origin.
Oversized rows retain their existing overlapping scroll behavior.

The resize test now spans enough height to change the page by at least one row
with smaller fonts. The completed-search test constrains its loading viewport
using the measured row height instead of assuming font metrics. Both keep their
original progress and visibility assertions. The download toolbar journey waits
for completed layout between paging keys; waiting for a new frame could time
out when the final boundary frame had already rendered.

The focused paging suite passed all **35 checks** with both Ubuntu DejaVu and
the local fonts. The download keyboard suite passed all **18 checks** with
DejaVu. Temporary font files and Fontconfig overrides are outside the repository;
the application's font selection is unchanged.
