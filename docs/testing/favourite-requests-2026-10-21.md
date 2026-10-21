# Favourite request safety — 2026-10-21

## Reproduction

A second capture replaced a still-open, unsaved favourite form. For a retained
or pending form, the primary engine-action controller kept its fields but
assigned the incoming bookmark ID afterward. The next save could therefore
target a different bookmark from the address and name visible in the form.
Metadata refreshes also reset the bookmark picker to its address-entry option
while the selected bookmark ID remained active.

Three focused regressions reproduced these failures. Inspection additionally
found that the secondary shell did not handle `start-page-bookmark` actions.

## Repair

Capture identity, address and name enter the editor together. New captures wait
while any form is open or retained, including before its first save. Waiting
requests are explicitly reviewed in arrival order, or dismissed one at a time.
They never replace the active draft or submit automatically. Duplicate waiting
captures are coalesced; sixteen can wait. Overflow rejects the new request with
a persistent explanation and a status message, preserving accepted requests.
Closing a tab with waiting work still requires the unfinished-work decision.
Profile changes and destruction clear the tab's waiting captures.

Both window shells pass bookmark IDs through the capture operation. Private
bookmark captures resolve only against their own temporary start-page state.
The picker now restores its selected index by ID after model changes and keeps
the name, caret, selection and address untouched. Missing bookmarks retain an
explicit unavailable choice. Removed or changed addresses disable saving until
the user chooses an address or another bookmark. A selectable, read-only field
shows the new address, with its beginning initially visible. Choosing an address
preserves the entered name.

The UI submits bookmark identity and its visible address through a typed
`save_favourite` method. The existing storage transaction validates that pair;
the backend no longer substitutes a newer cached address for the displayed one.
The private path checks the same pair against its current in-memory state.

The site list uses the existing focus-revealing scroll component. Cancel and
completion focus recovery ignore hidden controls and offer Review next after
the popup closes, while preserving focus already placed on another control.
An empty feedback-action row no longer reserves space. At larger text sizes,
compact headings and stacked view controls leave a usable content viewport.
Normal-width view names must still fit without clipping.

## Verification

The focused suite passes **66 checks**, including all original regressions,
unsaved/pending/failed/newer drafts, ordered review, cancellation, queue bounds,
profile reset, metadata reorder/removal/address changes, explicit recovery and
normal/large-text view controls. The 390-pixel/20-point and 320-pixel/26-point
renders have been inspected. The waiting control and changed-address choices
must fit their actual viewports. Start-page/runtime/shell lint reports 205
warnings and ten informational diagnostics, with no error/fatal diagnostics.

The full source suite passed **967 checks**, with no failures or skips in
195.364 seconds alongside compilation. Its log has no runtime type, reference,
assignment, binding-loop or unknown-signal errors. The release build passed in
**7m 58s**. The relinked favourite editor and request-safety journeys passed
**eight native checks** offscreen in 19.731 seconds, covering all three window
and profile cases with a 23-resource source-parity guard. The same combined
journeys passed **eight checks** through the owned Wayland seat in 20.232
seconds. Broader native qualification is in progress. Artifacts are retained in
`/tmp/ferric-favourite-requests-20261021/`.

Native fixtures use owned temporary profiles, SQLite databases and writer
locks, windows, D-Bus sessions and a compositor. Bookmark captures invoke the
production picker and engine-action path. Text entry uses Qt; fresh navigation
and action keys use the owned compositor seat in the Wayland gate. The explicit
old-address backend call separately tests identity validation below the form's
warning. These checks do not qualify desktop compositors, hardware input, GPU
rendering or platform IMEs.

## Cross-tab receipt retention follow-up

Review reproduced a closed draft losing Resume edit and its unfinished-work
close protection when another start-page tab saved a favourite. The bridge's
feedback lane belongs to the window, while the draft belongs to its tab.

Each tab now snapshots its own receipt's state, message and detail. The shared
receipt ID still gates incoming completions, but a later request from another
tab cannot retire the saved local result or its retained draft. Retry and
explicit cancellation continue to replace or clear that tab's request.

The added two-tab regression first failed for both a failed save and a newer
unsaved edit. Both now pass, along with an unchanged successful-save control.
It checks retention while the other save is pending and after it finishes,
local feedback and failure details, queued captures, keyboard Resume edit and
a fresh retry. The focused source start-page suite passes **69 checks**.
