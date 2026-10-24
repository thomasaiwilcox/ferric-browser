# Command editing and history — 2026-10-22

## Behavior

Up/Down recall previously executed commands. The old window handlers assigned
the recalled value directly to the command field, clearing Qt's native undo
stack. After typing a draft and recalling two commands, Ctrl+Z could not return
to either the preceding command or the draft.

The shared command surface now applies a recall as one native editor operation.
Both window shells use it. Ctrl+Z steps back through recalls to the draft and
its earlier typing; Ctrl+Shift+Z redoes those edits. Down still returns through
the actual profile-local history to the saved draft. An unchanged history
result leaves caret, selection and undo intact. Recall never submits a command.

Up/Down after Undo or Redo continue from a restored recorded command. Restoring
the saved draft returns to the draft position while retaining its session for
Undo. New unrecorded edits become the next draft. Explicit command entry starts
a fresh history browse without clearing
the visible editor text or its native undo state.

## Editor undo verification

The compiled baseline fails in primary, secondary and private windows. Each
case executes `zoom 110` and `zoom 120`, types `open unfinished-draft`, presses
Up twice, then Ctrl+Z. The field incorrectly remains `zoom 110` instead of
returning to `zoom 120`. The old production resources match source in this run.
The log is `target/command-history-native-baseline.log` (three behavioral
failures, two setup/cleanup passes).

The focused QML suite passes **26 checks**, including six new rows for native
Undo/Redo with typed, empty, Unicode and selected drafts, plus selection and
undo preservation at an unchanged boundary. Synchronous completion refreshes
remain enabled during the recall tests. The log is
`target/command-history-qml-fixed.log`.

The complete QML suite passes **1,122 checks** in **197.82 s**, using the
repository runner's `--disable-gpu --js-flags=--expose-gc` settings. The log is
`target/command-history-full-qml.log`. The initial invocation omitted the
forced-GC flag and reports 1,121 passes plus the required flag assertion's
failure; that unqualified run is retained in
`target/command-history-full-qml-unqualified.log`.

Production is frozen in `target/command-history-source-hashes.json`. Only the
command surface and its primary/secondary shell callers differ from the v6
Journey qualification. The all-target release build passes in **13 min 54 s**.
Compiled qualification uses the new embedded resources; the native fixture
retains its source/resource parity checks.

The focused compiled journey passes all **three window cases** in **7.65 s**
(five QtTest passes including setup/cleanup). The final regression uses
`zoom 1.1` and `zoom 1.2` so it can also require actual page zoom to remain at
1.2 throughout history editing. The log is
`target/command-history-native-history.log`.

The complete offscreen switcher gate passes in **166.88 s**. The Wayland editing
gate passes in **21.92 s**, including all three history rows. Logs are
`target/command-history-application_switcher.log`,
`target/command-history-application_command_editing_wayland.log` and
`target/command-history-wayland-editing-qt.log`.
The compositor-seat keyboard gate passes in **110.35 s** (**31 QtTest passes**),
including all three history cases and native press/release, modifier and focus
checks. Both Wayland gates include the new history cases alongside the existing
editing and navigation checks. Its logs are
`target/command-history-application_keyboard_chrome_wayland.log` and
`target/command-history-wayland-seat-qt.log`.

The initial release engine suite reports 499 passes, one ignored benchmark and
one obsolete source-string assertion. That assertion required the primary
window's old history cursor assignment, incidentally inside a test of navigation
prefill. The assignment now belongs to the shared editor. The assertion was
removed; the prefill value checks remain, and native editing tests check actual
caret/undo behavior. The corrected release unit rebuild passes in **8 min 38 s**,
and all **500 engine unit tests pass**, with one opt-in benchmark ignored.
The original result is `target/command-history-release-engine-tests.log`;
the final result is `target/command-history-release-engine-tests-final.log`.
The unit runner records success in `target/command-history-unit-status.json`.
That editor revision matched its production hash manifest at completion.

## History cursor and command-entry lifecycle

The editor correction revealed a separate cursor problem. After Up, Up, Undo,
the visible line is the more recent command but the backend still points at
the older one. Down leaves the recalled command in place instead of returning
to the draft; Up also fails to return to the older command. The 12-case native
baseline passes six cases and fails these two paths in all three window types
(eight QtTest passes including setup/cleanup, six failures, **20.31 s**).
The log is `target/command-history-cursor-native-baseline.log`.

A separate baseline restores the older command, presses Escape, re-enters
command mode and presses Up. It incorrectly remains at the older command.
All three window cases fail against the same preceding release, in **5.73 s**;
the log is `target/command-history-cursor-reentry-baseline.log`.

Previous and Next now share cursor reconciliation. An unchanged selected entry
keeps its position. After Undo/Redo changes the line, the saved draft takes
precedence over an equal recorded value; other exact recorded lines resume
at their own positions. An unrecorded edit detaches from history, and a fresh
explicit command entry resets the cursor and internal draft. Profile-local
history eligibility and persistence policy are unchanged.

Two pure Rust policy tests cover restored entries, Unicode, unavailable cursor
positions, absent browse sessions, new edits and matching draft values. A fast
preflight compiles the exact helper and those test bodies with `rustc --test`;
both pass in `target/command-history-cursor-model.log`. This preflight does not
qualify the Qt invokable or window behavior.

The first revised native matrix has **18 cases**: three window types and six paths
(Undo/Redo, next/previous after Undo, restored draft, edited history, new entry).
Production is frozen in `target/command-history-cursor-source-hashes.json`
with 41 files; only two Rust files differ from the previous qualified source.
All QML is unchanged from the 1,122-check run above. That release build passes
in **7 min 36 s**. Its native canary passes the primary and secondary rows but
hits the old 60-second harness budget before reaching private windows while
Rust compilation is active. The process is terminated by the harness; no
browser failure or complete-matrix pass is claimed for that run.

A further native case returns to the draft with Down, undoes that return, then
presses Down again. All three window cases fail against the first correction:
the field stays on the recalled command. The log is
`target/command-history-cursor-returned-draft-baseline.log` (**5.33 s**).
The model had used an absent cursor for both a new entry session and a session
that had returned to its draft.

The saved draft is now optional. Absence means history has not been opened;
a present empty string is a valid saved draft. Returning with Down retains that
session, allowing Undo to restore a recorded line and continue browsing from
it even while the selected-entry cursor is absent. Fresh entry, profile reset
and successful command recording clear the session as appropriate.

The final matrix has **24 cases**, adding Undo after return to empty and
non-empty drafts. The native canary budget is 180 seconds for this larger
matrix. The compositor-seat gate has a 180-second inner process budget and a
210-second outer CTest budget. No assertion or resource check was removed.
The exact helper and updated policy tests pass the Rust preflight in
`target/command-history-cursor-model-v2.log`.

The final snapshot is `target/command-history-cursor-v2-source-hashes.json`,
with 43 production files. Five Rust files differ from the earlier qualified
editor revision; QML remains unchanged. The all-target release build passes in
**7 min 24 s**. All **24 native history cases pass** in **55.80 s** (26 QtTest
passes including setup/cleanup), including both returned-draft boundaries in
every host. Logs are `target/command-history-cursor-v2-build.{jsonl,log}` and
`target/command-history-cursor-v2-native-history.log`.
The full offscreen switcher suite passes in **199.52 s** (**70 QtTest passes**).
The Wayland editing gate passes in **58.85 s** (**36 QtTest passes**), including
all 24 history cases. Detailed logs are
`target/command-history-cursor-v2-offscreen-qt.log` and
`target/command-history-cursor-v2-wayland-editing-qt.log`.
The compositor-seat gate passes in **149.66 s** (**52 QtTest passes**), including
all 24 history rows and native press/release, modifier and focus checks. Its
detailed log is `target/command-history-cursor-v2-wayland-seat-qt.log`.

All **502 release engine unit tests pass**, with one opt-in benchmark ignored,
in **0.53 s**. The result is
`target/command-history-cursor-v2-release-engine-tests.log`. The complete
qualification records success in
`target/command-history-cursor-v2-qualification-status.json`, after checking
the successful full build and all 43 production hashes again. QML and
JavaScript still match the previously qualified 1,122-check editor snapshot.

## Native editor gestures and input-method composition

The command field previously treated every Up/Down as a history gesture,
regardless of modifiers. Eight QML reference comparisons fail: Shift, Ctrl,
Ctrl+Shift and Alt with either arrow replace the draft with a recalled command,
while a plain Qt TextField retains the text. Six additional cases show that
Ctrl+Shift+N/P, Ctrl+Alt+N/P and Ctrl+Tab/Backtab incorrectly cycle completion.
The log is `target/command-editor-ownership-qml-baseline.log` (26 passes,
14 failures). A separate keypad control passes Up/Down and fails Right insertion
in `target/command-editor-ownership-keypad-baseline.log`.

The compiled browser reproduces Shift+Up draft replacement in primary,
secondary and private windows. Shift+Down happens to retain the text in this
idle-history setup; QML signal checks still reject the spurious history request.
The native preedit baseline also proves that Up/Down requests history,
Tab/Backtab and Ctrl+N/P move completions, Escape closes entry, and Return
submits while composition is active. Right and Space already defer correctly.
The combined baseline reports **7 QtTest passes and 11 failures** in **35.70 s**;
its log is `target/command-editor-ownership-native-baseline.log`.

The shared command surface now reserves exact modifier combinations for
completion, unmodified arrows for history, and treats keypad arrows equivalently.
Other modified keys use native editing. During preedit, the native editor and
input method keep the key event; an accepted signal cannot submit the command.
No Rust or history/persistence policy changes are part of this revision.

The QML runner has a test-only input-method injector that sends actual
`QInputMethodEvent` objects to the control. It does not fake the read-only
composition property. Ten source cases prove browser action signals remain
quiet while composing, then commit Japanese text and submit it with a fresh
Return. All **55 focused checks pass**, including modified/keypad arrows and
the prior history/undo cases, in **0.90 s**. The log is
`target/command-editor-ownership-qml-final.log`.

The native modifier cases use an ordinary Qt TextField in the same owned window
as their editing reference. Preedit cases observe history, completion, submission
and Escape signals and verify ordinary actions resume after composition ends.
The Return case checks the next command's exact text and actual page zoom.
These tests do not qualify platform IMEs, physical layouts or hardware.

The snapshot is `target/command-editor-ownership-source-hashes.json`: only
`FerricCommandLine.qml` differs from the preceding qualified 43-file snapshot.
The full QML suite passes **1,151 checks** in **217.23 s**, including the required
forced-GC stress test, in `target/command-editor-ownership-full-qml.log`.
The all-target release build passes in **14 min 08 s**. The focused native run
passes all **16 data cases** in **32.05 s** (18 QtTest passes including
setup/cleanup), against current embedded resources. Its log is
`target/command-editor-ownership-native-history.log`.
The complete offscreen switcher gate passes in **219.63 s** (**84 QtTest passes**).
Its detailed log is `target/command-editor-ownership-application_switcher-qt.log`.
The Wayland editing gate passes in **80.87 s** (**50 QtTest passes**); its detailed
log is `target/command-editor-ownership-application_command_editing_wayland-qt.log`.
The compositor-seat gate passes in **161.48 s** (**58 QtTest passes**), including
all six modified-arrow comparisons in real Wayland windows. Its detailed log is
`target/command-editor-ownership-application_keyboard_chrome_wayland-qt.log`.
The offscreen switcher budget grows from 240 to 300 seconds for 14 additional
native cases; all behavioral assertions and embedded-resource checks remain enabled.

All **502 release engine unit tests pass**, with one opt-in benchmark ignored,
in **0.52 s**. The result is `target/command-editor-ownership-release-engine-tests.log`.
The qualifier records complete success in
`target/command-editor-ownership-qualification-status.json` after verifying
the successful full build and all 43 frozen production hashes again.
