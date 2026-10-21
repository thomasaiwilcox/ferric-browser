# Favourite names — 2026-10-21

## Reproduction

Adding a favourite for an address already in bookmarks silently discarded the
name entered in the form. Selecting a bookmark also discarded changes to Name:
the backend substituted its old title before submitting the write. The UI
reported success and closed with a different name on the tile.

The compiled keyboard journey reproduced the mismatch in primary, secondary
and private windows. Two storage regressions also failed before the repair.

## Repair

The submitted name now reaches storage for typed addresses and selected
bookmarks. Pinning reuses the existing bookmark ID and writes its name and slot
in one transaction. Its creation timestamp is preserved; its update timestamp
advances. Replacing an occupied slot retains the former bookmark. Duplicate
addresses do not cause an unrelated bookmark to be renamed.

Private favourites update their bookmark and tile in memory. Explicit selection
requires the selected ID and URL to match, and an address match prefers an
already pinned bookmark before an unpinned match. Stale selections fail instead
of creating a replacement record.

The Name field explains that its value is shared with the bookmark, visibly and
through its accessible description. The explanation sits beside the field so
it remains visible when the form scrolls to Name at large text sizes.

## Verification

The storage suite passes **105 tests**. Its new cases cover selected and typed
addresses, existing pinned and unpinned duplicate records, stable identity,
creation/update timestamps and preservation of unrelated duplicates. A trigger
rejects insertion into the destination slot after the title update: both old
slots and bookmark names must remain intact, and a later retry must save the
new name and move the source reference.

The focused QML suite passes **47 checks**. A keyboard choice between two
bookmarks with the same address must submit the chosen ID and the edited name.
The narrow-window checks require the focused Name field and shared-name hint
to fit together at 390 pixels/20-point and 320 pixels/26-point text. Their
renders have been inspected.

The full source QML suite passed **948 checks**, with no failures or skips in
194.307 seconds alongside compilation. Its log has no runtime type, reference,
assignment, binding-loop or unknown-signal errors. Start-page lint reports 142
warnings and no error/fatal diagnostics. The release build passed in **8m 24s**.
The expanded favourite journey passed **five native checks** offscreen in
8.706 seconds. It re-pins a typed address with a new name, then chooses the
bookmark through the keyboard picker and changes its name again. Both paths
must preserve identity and a single bookmark/reference while publishing the
submitted title in the bookmark and tile. All three window/profile cases pass.
The same journey passed **five checks** through the owned Wayland seat in
9.703 seconds. Its captured error form was inspected: the shared-name
explanation remains visible with the retained draft and retry controls.
All five broad native gates passed **97 checks** in 284.59 seconds: 14
navigation/history, 43 offscreen keyboard, three Settings directory-sync,
12 Wayland editing and 25 compositor-seat keyboard checks, with no failures
or skips. The backend writer/selector-validation suite passed four checks in
5.816 seconds; real downloads passed 38 in 83.920 seconds; manager/library/
window/popup/session/startup workflows passed 21 in 69.576 seconds. This totals
**160 broad native checks**, plus the **10 focused native checks** above,
with no final failures or skips. Logs and renders are retained in
`/tmp/ferric-favourite-names-20261021/`.

Native fixtures use owned temporary profiles, windows, SQLite databases,
D-Bus sessions, loopback pages and a compositor. Fresh navigation/action keys
use the owned compositor seat in the Wayland gate; text entry and explicit
repeat events use Qt. These checks do not qualify a desktop compositor,
hardware input, GPU rendering or platform IMEs.
