# Tab and browser-surface keyboard continuity

Status: automated tab-focus qualification passed, 2026-10-23.

## Reproduction

The compiled browser could select the correct tab while leaving native focus
inside the hidden previous renderer. Closing the active tab could leave focus
on the window root. Secondary and private windows also lost page focus after
command and Switcher selection.

The initial native fixture has 15 cases across primary, secondary and private
windows: command selection, Alt+1 selection, Switcher selection, Ctrl+W closing,
and Escape after a tab changes behind Switcher. It loads two completed loopback
documents, establishes initial page focus once, then checks the actual document
and focus without repairing them. A subsequent Tab must reach the first page
button, and `j` must scroll that document.

`target/tab-keyboard-baseline-native.log` records 12 cases failing the page-focus
check. Its Qt totals include one additional failed case from the test's
press-delivery observer during Escape; Qt's modal WindowShortcut consumes
the press before QWindow key delivery. A standalone Qt reference confirms one
QWindow ShortcutOverride and one release, with the native scan code preserved.
The observer accepts this route only for Escape and retains exact press-count,
repeat, modifier and scan-code assertions. Other keys require QWindow KeyPress.
This is separate from the browser focus failure. Command and Switcher selection in the primary
window pass the original page-focus check.

Three new focused QML cases fail against the original controller: selected
page focus, current-page fallback in another window, and release during a
queued mode return. The baseline is `target/tab-keyboard-baseline-qml.log`.

## Change

The shared focus controller follows the selected view in each browser window.
It defers page focus until layout has updated, then checks the current view,
browser input ownership, modal scope, overlay stack and window lifetime. Other
uses of this controller, such as DevTools and link preview, do not opt into
following tab selection.

Secondary windows capture and restore command/search focus through the same
controller as the primary window. Overlay dismissal resolves a live fallback
from the invoking window when its captured page is hidden or destroyed.
Released windows cannot receive queued page or mode focus.

Seven focused QML cases cover tab changes, editor ownership, queued overlay
and modal protection, per-window fallback, and release. The full focused
suite passes 18 checks in `target/tab-keyboard-focus-qml-final.log`.

The native matrix also checks command and search cancellation without changing
tabs, and closing the first tab while its replacement keeps index zero, for
24 cases total. `application_tab_focus_wayland` uses the owned
compositor's keyboard seat for the same matrix. Native fixtures retain
source/resource parity checks, including the focus and tab controllers.

## Qualification

Production is frozen in `target/tab-keyboard-focus-source-hashes.json` (44
files). The all-target release build passes in 13 min 49 s. The full QML suite
passes 1,158 checks in 216.81 s, with the required forced-GC flags, in
`target/tab-keyboard-focus-full-qml.log`.

The focused compiled journey passes all 24 cases (26 QtTest passes including
setup/cleanup) in 56.45 s: `target/tab-keyboard-focus-native-focus.log`.
The same 24 cases pass with compositor seat keys in 56.82 s (26 QtTest passes):
`target/tab-keyboard-focus-application_tab_focus_wayland-qt.log`.
That gate requires native input metadata and cleared modifiers after each key.

The full Switcher gate passes 108 QtTest checks in 284.77 s. The existing
Wayland editing gate passes 50 checks in 80.88 s, and the compositor-seat
keyboard chrome gate passes 58 in 160.14 s. Their detailed logs are
`target/tab-keyboard-focus-application_switcher-qt.log`,
`target/tab-keyboard-focus-application_command_editing_wayland-qt.log` and
`target/tab-keyboard-focus-application_keyboard_chrome_wayland-qt.log`.
The Switcher budget is now 360 s for the 24 additional cases; this run passed
under its previous 300-second limit.

All 502 release engine unit tests pass, with one opt-in benchmark ignored.
The qualification runner records success in
`target/tab-keyboard-focus-qualification-status.json` and verifies all 44
production hashes at completion.

Additional Wayland recovery cases pass in primary, secondary, private and popup
windows. The combined recovery/Guide run finds two Guide article paging failures,
reproduced by a separate observed Guide run. Focus stays in the correct article;
its content height is stale until layout finishes. This independent
[first-scroll defect](guide-first-scroll-2026-10-23.md) is corrected and
qualified separately. The recorded focus revision and its source hashes remain
the completed baseline for that follow-up.

Physical keyboard layouts, assistive-technology traversal and a full desktop
compositor remain separate qualification work.
