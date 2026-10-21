# Library keyboard actions — 2026-10-20

## Reproduced problems

After selecting a row in History, Bookmarks or Quickmarks, Tab entered its Open
button. Shift+Tab then followed the native delegate focus chain into another
row's controls. It did not return to browsing the selected entry. All six
wide/compact cases failed this regression, covering 12 and 20 pt text.

At 390 pixels wide with 26 pt text, the Add to Favourites and private history's
Reopen in profile buttons extended beyond the list's horizontal viewport. Both
rendered-geometry regressions failed with the original buttons.

## Repairs

Shift+Tab from a row's first focusable control returns to browsing that row.
The next arrow key moves the selection immediately. This applies to Open,
an unfinished edit, a journey destination selector and reference-row Details.
Other controls retain their native backward traversal. Returning from an edit
does not save or discard the field's text; Tab can re-enter that editor.

Row buttons use the existing wrapping button component. Long labels fit the
available width and gain height as needed. Focused controls are scrolled into
view by the manager's existing focus revealer. Action callbacks, mutation
feedback and explicit deletion confirmation retain their existing contracts.

The library's full keyboard help explains the return gesture. The compact
library and switcher footers use `Tab · Esc`, which stays on one line at the
tested narrow 26 pt size. The Keys button retains the complete instructions.
Rendered captures were inspected after focusing each long button.

## Verification

The focused library suite passes 30 checks. It includes the six selection
regressions, an unfinished edit, journey/reference controls and narrow 26 pt
button geometry with focused-control visibility. The focused paging suite
passes 35 checks, including the visible switcher's compact footer. Footer
measurements explicitly select visible controls so hidden nested dialogs
cannot satisfy the check.

The native bookmark traversal now follows complete forward/backward paging by
selecting row 7, entering Open, returning with Shift+Tab and moving to row 8.
It runs through both Qt Test input and the owned compositor's keyboard seat.
The native resource comparison also includes the library manager component.

## Background-search paging follow-up

An interim release build passed in 7m36s, followed by a complete 798-check QML
run. The native bookmark and private-history journeys also passed, including
save failure/retry, deletion, quickmark editing and the new unsaved-edit focus
return. The compositor gate then exposed another first-page failure in a
secondary-window switcher. This interim run is not complete qualification.

The native key receipt confirmed focus on the intended field. At the exact
press, source state showed a fresh background batch pending, with the notice
Searching other browser windows. The temporary loading panel reduced the list
from 285 pixels to 142 pixels. Page Down read the last two pixels of the current
row, then completion of the batch expanded the viewport and contained that row
at its earlier offset. Waiting for a previous batch had not prevented a new
metadata-driven refresh during keyboard delivery.

Ordinary background work with usable rows now appears as an indeterminate
progress line in the search field. It does not change the result viewport.
Accessible descriptions retain the loading notice. Empty initial loading,
errors, details and retry actions retain the full feedback panel. The existing
source-state checks verify those paths and activation of still-current rows.
Two new 20/26 pt regressions failed the old viewport allocation (153 vs 285 and
111 vs 241 pixels); both now pass, including paging and Tab/Shift+Tab while
loading.

A separate pair of regressions reproduced Page Down using an old viewport size
after a source-details panel was hidden but before its parent layout polished.
The paging helper now polishes its ancestor layouts before measuring the page.
Both cases failed before the repair and now pass. Native key receipts retain
focus and source state at delivery, so future diagnostics can distinguish
background updates from an undelivered event.

## Final qualification

The final release browser and engine build passed in 7m39s on Qt/WebEngine
6.11.2. The complete QML suite passed **802 checks**, with no failures or skipped
cases, in 128.997 seconds. The final native keyboard gates then ran serially:

| Gate | Qt Test checks | Result |
| --- | ---: | --- |
| `application_switcher` | 30 | Passed |
| `application_command_editing_wayland` | 12 | Passed |
| `application_keyboard_chrome_wayland` | 12 | Passed |

All three passed in 171.37 seconds. Each compared seven compiled chrome
resources with the current source before running. Ordinary and uncached paging
passed in both windows, including the formerly failing secondary-window case.
The real persisted-bookmark case passed its row-7 Tab/Shift+Tab/Down journey
through the compositor seat. These gates use no script interceptors.

The separately rebuilt application executable passed `productPolishLibrary` and
`productPolishPrivateLibrary` (four Qt Test checks including setup/cleanup) in
9.788 seconds. It verified the unfinished editor's return/re-entry through the
actual BrowserUi, bookmark save failure and retry, deletion, quickmark editing,
background reads and private-history workflows. These four plus the three
keyboard gates total **58 native checks**, with no failures or skipped cases.

```sh
ctest --test-dir target/application-tests -R '^application_(switcher|keyboard_chrome_wayland|command_editing_wayland)$' --output-on-failure
env QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME= QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu target/application-tests/application_tests productPolishLibrary productPolishPrivateLibrary
```

Lint reported no critical/error/fatal diagnostics for the changed components;
the final diff whitespace check passed. The interrupted build, interim failed
compositor gates and instrumented diagnostic preflights are not counted as
successful release qualification. Native test resources came from the final
release build, with temporary interceptors removed.

The qualification uses software rendering and an owned Wayland compositor.
Storage workflows use Qt-generated input in owned offscreen windows; the seat
gate qualifies compositor-to-client keyboard delivery. Physical keyboards,
desktop compositor integration, platform IMEs and human accessibility evaluation
remain outside these gates.

The subsequent [library edit-draft follow-up](library-edit-drafts-2026-10-20.md)
repairs text loss after delegate recycling and catalog page changes, including
save acknowledgements delivered while an editor is offscreen. Its final run
passes 812 QML and 60 native checks.
