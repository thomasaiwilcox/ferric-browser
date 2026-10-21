# Settings editing — 2026-10-21

## Reproduction

Settings replaced its entire model on refresh or filtering. An unfinished
value disappeared when its row was filtered out, when another setting was
applied, or when a backend value changed. Even an unchanged refresh destroyed
the editor's focus, selection and undo history. Four focused regressions
failed before the repair.

Follow-up checks found four further failures: numeric/language formatting
prevented matching acknowledgments, reverting to the old value during an
in-flight submission was overwritten by its acknowledgment, and a rejected
toggle displayed a value the backend had not accepted. At 320 pixels with
26-point text, error feedback left too little viewport height for wrapping
draft actions. Another regression showed that backend row publication was
not observed without a manual refresh.

## Repair

The C++ settings model validates a complete snapshot before publishing it,
rejects empty/duplicate keys and reconciles rows by configuration key.
Surviving editors remain alive, including through insertions and moves.
Only changed roles are emitted. The row container restores a retained control
when ListView moves focus to a row during reconciliation; it respects modal
ownership and focus moved to another control.

Unapplied text belongs to the window's Settings owner, independently of row
widgets. Filtering, refreshing and closing/reopening retain it. Each edited
row shows an unapplied marker and a keyboard-accessible Discard edit action.
Discard restores the latest published effective value and returns focus to
the editor. Profile bootstrap or UI-owner replacement clears these drafts.

Submitted snapshots distinguish the acknowledged value from a newer edit.
Only a matching snapshot is cleared, including when the newer text happens
to equal the old effective value. Numeric literals and language lists are
compared with their published canonical representation. Rejected toggles and
enums return to their actual effective values. Held Return cannot resubmit
text fields.

The workspace observes its own backend row publication, including completion
of secondary-window writes. Toolbar and draft buttons wrap. The modal reserves
font-sized reading space and bounds error feedback so the editor and draft
actions remain reachable at large fonts.

## Verification

The focused Settings workspace suite passed **40 checks**. The full source
QML suite passed **907 checks**, with no failures or skips in 139.335 seconds.
Its log contains no runtime type, reference, assignment, binding-loop or
unknown-signal-handler errors. Lint reports 33 warnings and no fatal/error
diagnostics. The native test fixture compiles with an eighteen-resource guard.
The production release build passed in **7m 55s**.

The native journey drives the real backend from both window types at
20-point text. It types an edit, filters it away and back, submits a persisted
font change through Return, waits for the canonical published value, checks
that focus remains in its surviving editor, refreshes without losing a new
draft, discards through its actual button and reopens the retained draft.
It also injects an explicit held-Return event before the fresh activation.
The focused compiled journeys passed **4 offscreen** and **4 owned-seat
Wayland checks**. The fixture opens the supported `:settings` command and
types its query. It waits for the matching live configuration-key row rather
than caching a removed delegate while filtering settles.

Broader compiled-resource qualification passed **14 navigation/history**,
**40 offscreen keyboard**, **12 Wayland editing** and **22 compositor-seat
keyboard checks**. The real-download group passed **38 checks** in 83.764
seconds, and the manager/library/window/popup/session/startup group passed
**21 checks** in 70.099 seconds. This totals **147 native checks**, with no
failures or skips in the final gates. The release UI remained unchanged
throughout native qualification.

Screenshot qualification waits for a presented draft frame, then
[synchronizes Qt with the window system](https://doc.qt.io/qt-6/qguiapplication.html#sync)
before reading the committed buffer through the owned compositor. These
synchronization and capture calls are confined to the test fixture.
The final Settings/Downloads render rerun passed **6 checks** in 13.506
seconds. Primary and secondary committed-frame review copies both show the
new `24` draft, its Discard action and their independent profile labels.
The 390-pixel/20-point and 320-pixel/26-point error renders were also inspected.

Logs and renders are retained in `/tmp/ferric-settings-drafts-20261021/`.
All native fixtures own their profiles, configuration, windows, D-Bus session
and compositor; they do not operate personal browser state or desktop input.

Follow-up [save-feedback qualification](settings-save-feedback-2026-10-21.md)
corrected the persistence-scope assumption in this journey. The earlier
secondary-window check inspected the primary before asynchronous peer
synchronization. Saved runtime settings intentionally share a state root
across regular profiles; the current journey waits for that synchronization,
and Settings now states the shared scope explicitly. Session overrides remain
local to their window.
