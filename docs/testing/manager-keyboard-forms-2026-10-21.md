# Manager keyboard forms — 2026-10-21

## Reproduction

Return in the session name, profile display name and profile rename fields did
nothing. Return in the new profile name also failed to advance to its display
name. Cancelling a profile rename left focus on the hidden form. Five new
Qt-key regressions failed before the repair.

Replacing profile or session rows while editing moved focus from the text
field to a new row. Both focused model-replacement regressions failed.

At 320×600 with 26-point text, the fixed toolbar left a 55-pixel session field
inside a 37-pixel viewport. The profile viewport was 56 pixels high, and its
focus padding also clipped the input. Toolbar and feedback controls could
extend beyond the panel horizontally. Both large-text form checks failed.

## Repair

Return advances from a nonempty profile name to its display name. Return in
the display name, rename field or session name submits the same request as its
button. These paths respect blank fields, pending operations, private-session
restrictions and modal ownership. Repeated Return events do not advance or
submit. Request signals avoid programmatically clicking a button during the
original field event, which moved focus and caused duplicate activation.

Completing profile creation clears its matching draft and prepares the next
name when focus is still in that form. It preserves focus when the user has
moved elsewhere. Closing rename returns focus to its invoking control when
available, with the persistent new-profile name field as fallback. Its queued
focus request checks current modal ownership.

The library's header-focus protection now lives in the shared manager list.
Catalog publication preserves the active, enabled header field or control;
explicit browsing and row-action navigation clear the retained target.

Profile and session toolbars now scroll with their form header. Buttons wrap
inside the available width, including shared operation-details controls. The
minimum form viewport follows the font size so a two-line button remains
usable. The session viewport in the 26-point fixture is now 116 pixels high.

## Verification

The focused manager suite passed **38 checks**, including empty/pending/private
submission guards, matching receipts, rename cancellation and completion,
model replacement, normal Tab traversal, and 320-pixel/26-point forms. The
library suite passed **71 checks** after focus protection moved to the shared
list. The Settings suite passed **20 checks**; its newly visible feedback
control is laid out and activated through Space before checking literal error
details. Both large-text renders were inspected: controls wrap inside the panel
and the focused save action remains visible. Lint reports warnings but no
critical/error/fatal diagnostics.

The complete QML suite passed **869 checks** with no failures or skips in
194.993 seconds, including the rename-publication focus regression and the
initial-content and popup-handoff changes. The full log has no runtime
type, reference, binding-loop or assignment errors.

The native fixture checks create, rename cancellation, rename submission and
session saving in primary and secondary windows at 20-point text. Fresh
navigation and Return use the owned compositor seat in its Wayland gate;
text and explicit repeat events use Qt. It checks actual key recipients,
published catalogs, draft cleanup, receipt ownership and focus after completion.
The compiled-resource guard now compares thirteen chrome resources, including
both window components.

The final release build passed in **8m 15s**. Both native test programs were
relinked against it. The native form journey closes Profiles through its actual
Close button and waits for a presented Sessions frame before typing. Primary
and secondary native renders, plus compact secondary profile/session renders,
were inspected.

Broader qualification also exposed and repaired a
[first-navigation history mismatch](first-navigation-2026-10-21.md).
The final native gates passed **36 offscreen**, **12 Wayland editing**,
**18 compositor-keyboard** and **14 history/navigation checks**. The additional
manager, library, window, popup, session-preview and startup-confirmation run
passed **21 checks**, for **101 native checks** total. There were no failures or
skips, and the logs contain no runtime type, reference, binding-loop or
assignment errors. Detailed timings and popup coverage are in the linked
qualification record. Logs and renders for this pass are in
`/tmp/ferric-manager-keyboard-20261021/`.
