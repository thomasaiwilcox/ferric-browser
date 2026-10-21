# Library edit drafts — 2026-10-20

## Reproduction

An unfinished bookmark title or quickmark address disappeared after leaving its
editor, moving to the end of a long list and returning. The ListView destroyed
the original row outside its viewport. Both regressions failed with caching
disabled and the original row confirmed absent; returning created a fresh row
with no editor or draft.

The earlier background-read checks retained the original delegate, so they did
not cover this lifecycle. The text and editing flag lived in the row widget.

## Repair

Draft text now belongs to the window's library component, keyed by record kind
and identity. Recreated rows derive their editor state from that owner. Normal
scrolling, page changes and switching library kinds retain unfinished text.
Identically named bookmarks and quickmarks keep separate drafts. Replacing the
catalog object or owning browser window clears this transient state.

Drafts remain in memory for the owning component's lifetime. Catalog snapshots
are bounded, so absence from a displayed page is not used as evidence that a
record was deleted. The existing explicit Save path submits the mutation.

A successful matching save clears the submitted value even when its row is
offscreen. A failed save retains the draft; an older success cannot clear a
newer value. Cancel edit and an acknowledged deletion discard the corresponding
draft. An active editor's successful-save focus return still finishes on its
Edit button.

The implementation retains text rather than keeping offscreen row widgets
alive. The regression verifies the original delegate was recycled before
checking the restored editor.

## Verification

The focused library suite passes 40 Qt Test checks. Coverage includes bookmark,
quickmark and empty drafts; recycling; cancellation; successful and failed
receipts while offscreen; newer edits; record-kind separation; and replacement
of the catalog owner, plus native undo/redo while draft tracking is active.
Existing editing, deletion, modal-focus and large-font
checks pass in that same run. Lint reports no critical/error/fatal diagnostics.

The new native journey seeds 110 real bookmarks, edits row 7 through the UI,
evicts that row, changes catalog pages, returns to the draft and explicitly
saves it through BrowserUi. It runs offscreen and in the owned compositor's
keyboard gate. Text entry uses Qt-generated events; navigation and action keys
use the seat in the compositor gate. Its targeted run against the release
resources passed all three checks, including setup and cleanup.

The release browser and engine build passed in 7m54s on Qt/WebEngine 6.11.2.
The complete QML suite passed **812 checks**, with no failures or skipped cases,
in 131.108 seconds. The final native gates ran serially after linking:

| Gate | Qt Test checks | Result |
| --- | ---: | --- |
| `application_switcher` | 31 | Passed |
| `application_command_editing_wayland` | 12 | Passed |
| `application_keyboard_chrome_wayland` | 13 | Passed |

These three gates passed in 177.51 seconds and each verified seven compiled
chrome resources against the current source. They include the new draft journey
alongside ordinary and uncached paging, completion editing and action focus.
No source overrides or script interceptors were used.

The separately rebuilt application executable passed `productPolishLibrary`
and `productPolishPrivateLibrary` in 9.747 seconds: four Qt Test checks including
setup and cleanup. Its real storage worker exercised save failure/retry,
deletion, quickmark editing, background reads and private-history transfers.
Together these runs passed **60 native checks**, with no failures or skipped
cases. Fixture-runner Python compilation and the final diff whitespace check
also passed.

```sh
ctest --test-dir target/application-tests -R '^application_(switcher|keyboard_chrome_wayland|command_editing_wayland)$' --output-on-failure
env QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME= QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu target/application-tests/application_tests productPolishLibrary productPolishPrivateLibrary
```

These are software-rendered owned fixtures. The seat gate qualifies
compositor-to-client key delivery; it does not cover physical keyboards,
desktop compositor integration, platform IMEs or human accessibility evaluation.
