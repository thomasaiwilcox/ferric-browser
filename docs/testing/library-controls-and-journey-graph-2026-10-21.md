# Library controls and journey graph — 2026-10-21

## Reproduction

The Relationships view accepted only 40-pixel up/down steps. Page Up/Down,
Home and End did nothing, including when the map header overflowed without
relationship rows. Four real-key regressions failed before the repair.

A single-node map also reserved four columns and let Right pan into empty
space. The wide-window regression moved 97.545 pixels despite the node fitting
entirely in its viewport. A fixed-height sparse map could put its only node
above the visible area at End. Dense maps could also make their own viewport
taller than the outer reading area, leaving lower rows clipped during panning.

At 390×600 and 26-point text, the fixed library toolbar left only 34 pixels for
the relationships viewport. Its long buttons could also exceed the available
width. The controls competed with the content they were meant to operate on.

Returning from a header control exposed a separate focus-scope issue: focusing
the ListView retained its focused descendant, so navigation could remain on
Refresh or the search field instead of returning to browsing.

Replacing library rows could also move focus from a header control to the new
current row. A regression focused Refresh, replaced the records and pressed
Return; the old implementation opened the selected page instead of refreshing.
Wayland key receipts exposed the same focus change during page navigation.

Full traversal through fourteen persisted relationships exposed a fractional
content-height estimate: Page Down stopped with the last row's bottom at
358.607 pixels in a 357-pixel viewport. The matching QML regression reproduced
the clipping at 358.429 pixels.

## Repair

Library toolbar controls now share the list's scrolling header. Buttons and
page labels wrap within the available width. Large text leaves space for the
results, and focused controls scroll into view.

Shift+Tab from list browsing reaches the header controls. Leaving the first
toolbar control backward restores browsing and the selected entry; Tab from
the final toolbar control returns to the selected entry's actions when no
other header controls follow it. Returning to browsing explicitly clears
descendant focus in the ListView's focus scope. Ordinary row-action Backtab
uses the same focus operation.

Model updates preserve the active, enabled header control and its focus reason.
Explicit navigation back to browsing or row actions clears that reservation.
When the final row is instantiated, paging uses its actual bottom instead of
the ListView's estimated content length.

Empty library lists can page through long feedback and use Home/End while
keeping selection at -1. Navigation measures the library surface after its
text and layout have reflowed. The resize fixture acknowledges the asynchronous
native window geometry before changing text size and delivering the key.

Relationships now supports Page Up/Down with visible overlap, Home/End and
g/G. Navigation cancels inertia and supersedes a queued startup scroll reset.
Search-field editing and focused-map arrow panning keep their own behavior.
The map's horizontal extent uses its populated columns, with a four-column
limit. Its height fits both its contents and the outer reading viewport.
Focused maps support their own Page Up/Down, Home/End and g/G without moving
the relationship outline. Small maps no longer expose unused grid columns or
reserve several screens of empty vertical space.

## Verification

The focused library suite passed **71 Qt Test checks**. New checks cover
viewport paging, both end shortcuts, g/G, maps without relationships, map
bounds, search editing, return from search focus, toolbar focus in both
directions, selection retention, large-text controls, resized text layout,
empty lists with long feedback, preservation of header focus during row
replacement, and paging all the way to a fully visible final relationship.
The shared paging suite passed **35 checks** and the modal/chrome interaction
suite passed **46 checks**. Lint reports no
critical/error/fatal diagnostics. The complete QML suite passed **843 checks**
with no failures or skips in 180.589 seconds alongside the release build, and
its log has no runtime type, reference, binding-loop or assignment errors.
Supplemental map g/G checks at
20 and 26 points passed four checks including setup/cleanup.

The native `journeyGraphKeyboard` fixture records fifteen owned loopback visits,
opens Relationships through its real toolbar button, reaches the map through
Tab traversal, pans and pages it, then returns to browsing with Shift+Tab. It
traverses the relationship outline in both directions with Page Up/Down at
20-point text, checking visible overlap and both end markers, followed by
Home/End and g/G. Seed pages must render their requested document URL and
persist their journey record first.

The persisted-bookmark native journey also checks Shift+Tab to the scrolling
toolbar, return to browsing, and Tab back to the selected entry's controls.
The draft-preservation journey also reaches Next and Previous through reverse
Tab traversal and records the actual recipient of Return on each control.
These journeys run offscreen and through owned Wayland seat events. Native
qualification compares nine compiled chrome resources against the current
source and rejects internal QML type/reference/binding-loop warnings.

Final release build passed in **8m 13s**. Both native test programs were relinked
against that archive before qualification. All three native gates passed:

- `application_switcher`: **34 checks**, 111.372 seconds.
- `application_command_editing_wayland`: **12 checks**, 16.481 seconds.
- `application_keyboard_chrome_wayland`: **16 checks**, 64.587 seconds.

The persisted-library and private-library storage journeys passed **4 checks**
in 9.839 seconds, for **66 native checks** across these gates. There were no
failures or skips, and the native logs contain no runtime type, reference,
binding-loop or assignment errors. A targeted Wayland run of library paging,
draft preservation and relationship navigation also passed all five checks
including setup/cleanup.

Inspected the 390×600, 26-point toolbar captures in Outline and Relationships,
the large-text map render, and native 390×700, 20-point relationship controls
and final-row renders. Wrapped controls remain inside the surface; the final
relationship is fully visible. Captures and test logs are in
`/tmp/ferric-journey-graph-20261021/` for this session.
