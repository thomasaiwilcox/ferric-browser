# Favourite editor — 2026-10-21

## Reproduction

Return in the address field saved immediately, before the user reached Name.
The dialog closed when an asynchronous write was merely accepted, losing the
editable context if storage later failed. Its scroll area did not reveal the
focused address at 390 pixels/20-point text or 320 pixels/26-point text, and
the fixed footer overflowed. Six added regression cases reproduced these
failures before the repair.

## Repair

Return advances from Site address to Name. Return in Name or Save favourite
submits once; held Return is suppressed. The editor reveals focused controls,
and footer actions wrap within its available width.

Favourite mutations publish an owned receipt with an ID, progress, result and
plain-text failure detail. A durable receipt follows its actual storage ticket,
then the metadata read submitted after that write. An earlier metadata read
cannot settle it. Private saves settle synchronously. Profile changes clear
the request and local form. A committed write whose refresh fails remains
reported as saved, with a refresh warning and details.

The form stays open during saving and on failure. The submitted text can be
retried after failure, while newer text survives an earlier successful save.
A newer name following a successful new pin becomes an edit of that saved
bookmark. Close during saving retains the draft and exposes Resume edit.
Cancellation after the request finishes discards the local form. Successful
unchanged submissions return focus to their saved or moved tile.
If a move finishes after closing its form, grid focus follows the destination;
focus already placed on another page control is preserved. A two-case regression
reproduced the old-slot focus failure and checks both outcomes.
The compiled run additionally reproduced grid focus restoration pulling focus
out of the newly opened editor in all three windows. `handleNavigation` opened
the dialog and immediately called `focusContent`. The start page now rejects
content-focus requests while a dialog or menu is open, or while the page is
hidden. Focused regressions cover Enter on an empty tile and later explicit
focus requests with the address or name active.
The broader native run found a second focus race: the queued save-completion
callback could pull a quick Tab from Options back to its tile. Completion now
requests grid focus only while no other page control owns input. Its focused
regression failed before that repair and passes afterward.

Start-page and wrapping buttons accept a fresh Return or keypad Enter through
Qt's public button `click()` method, preserving checkable control behavior.
This also makes Resume edit and Show details usable with Return. Source tests
wait for the controls' actual geometry before asserting clicks and toggles.
The [Qt 6.11 button API](https://doc.qt.io/qt-6.11/qml-qtquick-controls-abstractbutton.html#click-method)
documents that activation method.
The first generic Enter handler conflicted with toolbar navigation handlers:
one handler activated the button, another marked Return unhandled, allowing
the same key to reach browser routing. A native Library page-return failure
showed one Previous click, no Close click, and a closed/cleared library.
Two focused regressions reproduce the escaping Return/keypad Enter. The
buttons now use specific `returnPressed`/`enterPressed` signals, which Qt handles
before generic `pressed` navigation handlers. See
[Qt's key handling order](https://doc.qt.io/qt-6.11/qml-qtquick-keys.html#key-handling-priorities).

## Verification

The focused source suite passes **46 checks**. The two narrow, large-text
renders were inspected with the address field focused and the footer visible.
Lint reports 142 warnings and no error/fatal diagnostics across the start page
and its two button components.

Final production source passed **947 full-suite checks**, with no failures or
skips in 204.414 seconds alongside compilation. Its log contains no runtime
type, reference, assignment, binding-loop or unknown-signal errors.
Logs and renders are retained in
`/tmp/ferric-favourite-editor-20261021/`.

The final release build passed in **8m 19s**. The relinked favourite and Library
regression journeys passed **six checks** offscreen in 10.599 seconds, covering
primary, secondary and private favourite windows with a 22-resource source-parity
guard. The native fixture waits for a presented frame and positive Options
geometry after save completion before sending Tab. This lets Qt finish popup
closing and presenting the new control; the separate source regression still
checks a quick control-focus change before the application's deferred callback.
The same combined journeys passed **six checks** through the owned Wayland
keyboard seat in 11.185 seconds. Library Previous must click once, Close must
not click, and all 100 first-page records plus the retained draft must return.
Committed-frame review confirms the retained name, selectable SQLite failure
and retry action remain visible in both regular windows.

The five broad native gates passed **97 checks**: 14 navigation/history,
43 offscreen keyboard, three Settings directory-sync warning, 12 Wayland
command-editing and 25 compositor-seat keyboard checks. They had no failures
or skips and completed in 283.99 seconds. The final backend receipt run passed
four writer/selector-validation checks in 5.478 seconds. Real-download tests
passed 38 checks in 84.629 seconds, and the remaining manager/library/window/
popup/session/startup workflows passed 21 in 69.994 seconds. This totals
**160 broad native checks**, in addition to the **12 focused native checks**
above, with no final failures or skips. The switcher suite's aggregate CTest
limit allows the added browser journeys; individual assertion deadlines remain
unchanged.

The native fixture uses owned temporary profiles, SQLite connections, windows,
D-Bus sessions and a compositor. Its writer lock is released before SQLite's
five-second busy timeout; a temporary trigger then rejects a real bookmark
update, and removing the trigger permits a fresh keyboard retry. The ordered
read/submission step invokes the actual focused field's acceptance signal in
one event-loop turn. Other fresh Return/navigation events use the owned seat
in the Wayland gate; text entry and explicit repeat events use Qt. The final
tile must load the actual loopback document. These checks do not qualify a
desktop compositor, hardware input, GPU rendering or platform IMEs.
