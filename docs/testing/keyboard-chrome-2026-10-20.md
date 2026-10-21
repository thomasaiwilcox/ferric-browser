# Keyboard chrome usability repair — 2026-10-20

This pass exercises the command picker and universal switcher using original
Qt keyboard events and rendered production QML. It is one part of the ongoing
keyboard-first Linux usability audit, not a daily-driver qualification.

## Reproduced problems

- Cycling command completions could select a row outside the visible popup.
  The popup had no current-index binding or selection reveal behavior.
- A native Backtab event did not cycle backwards. Qt can represent Shift+Tab
  as Backtab; only the Tab representation was handled.
- Completion titles and descriptions used Qt's default text font instead of
  the configured browser font. The regression fixture observed 9 pt text with
  the browser configured to 26 pt.
- Home and End in the switcher search field moved the result selection instead
  of the text cursor. Shift+End also failed to select the query text.
- A window-wide Ctrl+p shortcut reopened the switcher, clearing the current
  query and overriding command completion and the configured tab-pin binding.
  This conflict was reproduced by the composed application test after the
  isolated component tests passed.
- Ctrl+Space reached its configured `switcher` binding but failed with a generic
  command error: parameterless `switcher` was incorrectly routed to the
  core-only dispatcher. It now uses the same full executor as the command bar.

The new QML regressions failed against these original behaviors before repairs.

## Behavior after repair

Tab/Shift+Tab and Ctrl+n/Ctrl+p keep the selected completion in view, including
wraparound and window resizing, while focus stays in the command field.
Completion labels use the configured font and rows accommodate both text lines.
The list has a scrollbar when its contents exceed the popup.

The switcher preserves native Home/End and Shift selection in the query field.
Ctrl+Home/End select the first/last result; arrows and Ctrl+n/Ctrl+p select
adjacent results. Its keyboard help and README describe these gestures.
The duplicate window shortcut is removed; Ctrl+p follows the configured mode
binding or the focused command/switcher field. Ctrl+Space still opens the
switcher through the binding registry.

## Verification

The focused QML tests cover forward/backward cycling, both Shift+Tab event
representations, wraparound, focus retention, resizing, large-font geometry,
query cursor movement/selection, and result activation after first/last jumps.

The production application regression additionally covers command completion
through each window's real BrowserUi in primary and secondary windows, and
switcher search editing and Normal-mode Ctrl+p tab pinning through the composed
keyboard router. Run it with:

```sh
cargo build --release -p ferric-browser-engine-qt -p ferric-browser --locked
cmake --build target/application-tests --target switcher_tests --parallel 2
ctest --test-dir target/application-tests -R '^application_switcher$' --output-on-failure
```

For the complete QML suite:

```sh
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software \
QTWEBENGINE_CHROMIUM_FLAGS='--disable-gpu --js-flags=--expose-gc' \
target/qml-tests/qml_tests -input crates/ferric-browser-engine-qt/tests/qml
```

Qt/WebEngine version: 6.11.2. These checks use the offscreen software backend.
They do not establish physical compositor input, platform IME behavior,
screen-reader usability, or compatibility with authenticated live sites.

Final results:

- Release browser and engine build passed.
- Complete QML suite: 738 passed, zero failures or skips.
- Production application switcher suite: 15 passed, zero failures or skips;
  CTest passed in 22.72 seconds. This includes the formerly failing default
  Ctrl+Space gesture after pinning/unpinning a tab.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- QML lint reported no critical/error/fatal diagnostics for the three changed
  production QML files. Advisory diagnostics remain.
- The completion popup render was inspected after scrolling to the last item
  and reducing the window height; the selected item remained fully visible.

The routing-policy unit regression also names parameterless `switcher` in the
full-executor list. The Rust unit suite was not run in this pass; the production
application test independently verifies the actual bound command path.

## Remaining audit work

Continue actual browsing journeys and native Wayland input qualification.
The follow-up [list paging repair](list-paging-2026-10-20.md) addresses the
switcher's fixed eight-result jumps and library row-height estimates.
The [completion editing follow-up](command-completion-editing-2026-10-20.md)
also repairs insertion, native selection synchronization and mouse-focus loss
during a synchronous candidate refresh. Their final qualification includes a
785-check QML run and 54 native checks, including compositor-delivered keyboard
events in an isolated software-rendered Wayland session.
The later [library keyboard actions follow-up](library-keyboard-actions-2026-10-20.md)
adds backward return to browsing, wrapping action labels and a stable result
viewport during background search. Its final run passes 802 QML and 58 native
checks.

## Repository contract follow-up — 2026-10-21

The source-contract assertions now check the Settings submission helper and
viewport-based switcher paging. The accessibility gate follows the result
count's dynamic description while still requiring its idle label. A runtime
QML regression checks the idle description, pending-search fallback, explicit
source notice and return to idle.
