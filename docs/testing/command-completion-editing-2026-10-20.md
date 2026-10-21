# Editing selected command completions — 2026-10-20

The keyboard usability audit found that selecting a command completion did not
provide a keyboard path to insert it and continue editing. For example, after
selecting `open` for `op`, Space left `op ` in the input and Right did nothing.
Enter executed the selection instead of allowing arguments to be appended.

Mouse insertion also read the replacement range and candidate array again after
editing the input. Qt text/cursor notifications synchronously refresh those
properties through BrowserUi. With `op --target tab`, clicking `open-current`
produced a JavaScript exception and left the cursor after the trailing arguments.
The original application failed all six primary/secondary Right, Space and
mouse cases. Both native preedit controls passed on that baseline.

## Repaired behavior

Tab and Shift+Tab retain their suggestion-selection behavior. At the end of
the input, Right inserts the selected suggestion without running it. Space
inserts it and lets the original native Space event begin the next word.
Return remains the execution gesture. Middle-of-input arrows, modified arrows,
text selections and active input-method composition retain native editing.
Hidden suggestions behind command feedback cannot be inserted by these keys.

The completion edit captures its value, range, resulting text and UTF-16 cursor
before changing the native field. Native remove/insert operations preserve undo
history, with a cursor boundary separating completion from preceding typing.
Undo restores the original text and caret without leaving a text selection.

The popup shows its insertion/execution keys. A compact hint is used in narrow
windows; a short large-font popup can grow enough to show a full suggestion and
the hint. Suggestion names occupy the first line, with their category and detail
below, so a category prefix does not hide the name in narrow windows.

## Verification

The QML regression covers synchronous candidate refresh, trailing arguments,
UTF-16 cursor placement, Right/Space editing without submission, cursor/selection
preservation, native undo/redo after typing and deletion, feedback visibility,
and the rendered narrow 26 pt popup.

The application regression sends original Qt key and mouse events to the shipped
QML in primary and secondary windows. Keyboard cases append a loopback URL and
execute the resulting `open` command, requiring the real WebEngine view to load
that address. Mouse cases preserve trailing arguments, caret position and undo.
Explicit Qt preedit events check that Right/Space do not accept a completion
while composing. These events do not qualify a physical platform IME.

```sh
cargo build --release -p ferric-browser-engine-qt -p ferric-browser --locked
cmake --build target/application-tests --target switcher_tests --parallel 2
ctest --test-dir target/application-tests -R '^application_switcher$' --output-on-failure
ctest --test-dir target/application-tests -R '^application_command_editing_wayland$' --output-on-failure
```

The automated environment is Qt/WebEngine 6.11.2 with offscreen software
rendering and disposable browser roots. A separate Wayland gate uses the existing
owned Qt compositor, keyboard seat and private D-Bus session. The command-editing
checks run against Wayland client windows using Qt-generated key/mouse events;
they do not prove hardware/compositor-delivered input. Physical keyboard delivery,
platform IMEs and authenticated live-site browsing remain separate parts of the
ongoing product audit.

Final verification against the release resources:

- Release browser and engine build passed.
- Complete QML suite: 766 passed, zero failures or skips.
- Application switcher suite: 26 passed, zero failures or skips; CTest passed
  in 70.44 seconds.
- Wayland command-editing gate: 12 passed, zero failures or skips; CTest passed
  in 16.31 seconds.
- The portal request regression also passed after the shared fixture runner was
  extended. All fixture XDG directories, including state, use temporary roots.
- Formatting, diff whitespace, runner syntax and QML lint's critical/error/fatal
  diagnostic gate passed. The narrow 26 pt completion popup render was inspected.

The broad application run initially exposed intermittent failures in the paging
journeys introduced earlier in this audit. The fixture now supplies its large
font at startup, waits for a presented initial layout, and requests a new rendered
frame after each delivered key before measuring page boundaries. It also checks
window activation, focus ownership and list readiness explicitly. These checks
do not retry a dropped key or relax traversal assertions.

A secondary-window no-progress result was still observed during investigation
with the correct window and search field focused. Four later focused runs and
the final full application run passed; the cause of the earlier intermittency
has not been established. Keep that investigation open. Completion-editing
checks passed independently in both windows and both tested platform paths.


## Delegate lifetime follow-up

A later release-resource run exposed a second synchronous-refresh issue. After
applying the edit, the clicked delegate tried to call `commandSurface.focusInput`.
The model refresh had already invalidated its QML context, producing
`ReferenceError: commandSurface is not defined`. The original text assertions
passed because the command field was already focused. A QML case that first
moves focus away reproduced actual failure to restore keyboard editing.

The complete mouse gesture now runs in a method on the command surface, whose
context survives replacement of the completion rows. The delegate forwards its
index in one call. Selecting the candidate, applying the captured edit, and
restoring focus all finish in the surviving owner.

The application suite now treats internal resource `ReferenceError`, `TypeError`
and binding-loop warnings as failures. Both primary and secondary mouse cases
failed under that stricter gate before the repair. The focused QML completion
suite passes 19 checks including the new lost-focus/model-refresh case.
Switcher row and button activation were also checked for synchronous delegate
removal; those three activation cases already passed without a source change.

The initial full QML run during the wider follow-up was invalidated by an
incorrect lint output path and a missing forced-GC test flag. The source was
restored to its exact earlier contents, the lint destination was corrected, and
subsequent full runs and compiled-resource parity checks passed. Those initial
failed runs are not counted as successful qualification.


## Completion model replacement follow-up

A later compositor run failed its current-row visibility check after wrapping to
the last completion. A deterministic same-count description refresh reproduced
a state mismatch in QML and both application windows: the bridge still selected
row 99 while ListView reset its native current index to 0. The native geometry
snapshot showed that the highlighted row 99 itself was still visible. The older
check had assumed that the native current item and bridge selection agreed.

The popup now explicitly synchronizes its native current index from the bridge
when selection, model or current-item creation changes, then finishes layout and
reveals that selection. This also covers asynchronously materialized rows. The
native regression requires both index agreement and a fully visible selected
row, and injects a same-count presentation refresh in each window. Candidate
values and command execution are unchanged by that fixture refresh.

The original component failed the same-count QML case even after waiting, and
both native windows failed with `selected:99,current:0`. The focused completion
suite now passes 20 checks. The final release-resource run is recorded below.

## Final release-resource qualification

The release browser/engine build and complete **785-check QML suite** passed
with the final product source. All three application gates then passed serially:
30 offscreen switcher checks, 12 Wayland editing checks and 12 compositor-keyboard
checks, **54 native checks total**, with no failures or skipped cases. Each gate
verified the compiled chrome resources against the source files before running.

Both windows passed completion index/visibility checks after a same-count model
refresh. Right, Space, mouse insertion, trailing arguments, caret and undo passed
through the real BrowserUi/WebEngine application. The original mouse gesture is
observed after the chosen delegate is rendered, and internal QML reference/type
errors or binding loops fail the gate. Compositor-delivered Tab/Shift+Tab and
Control+N/P retain focus while cycling suggestions. The explicit preedit checks
remain Qt-generated composition controls, not platform input-method coverage.

See the [paging qualification](list-paging-2026-10-20.md#final-release-resource-qualification)
for gate details and limits. Lint had no critical/error/fatal diagnostics, and
formatting, fixture-runner Python compilation and diff whitespace checks passed.
