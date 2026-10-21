# Journey reopen destinations — 2026-10-20

## Reproduction

Choose New tab or New window for a journey entry, press Shift+Tab to return to
list browsing, then press Enter. The destination selector retained the chosen
label, but Enter always reopened the entry in the current tab. Both keyboard
regressions failed before the repair: the emitted target was `current`.

The Reopen button and list activation used different sources for their target.
The selector also kept its choice inside a recyclable row widget.

The first native screenshots exposed clipped destination text at a 390-pixel
window width and 20-point text: New tab and New window appeared as partial
words. A rendered-text width regression also failed at 20 and 26 points.

## Repair

Enter and Reopen now read the same destination, stored by canonical journey
node identity in the owning library component. Display record IDs and titles
can change without changing that choice. Another node keeps its own default.
Scrolling out of view and back restores the choice after the original delegate
has been destroyed. Choosing Current tab removes that node's override.

This is transient state for the component's lifetime. Replacing the catalog
object or owning browser window clears it. Ordinary catalog content updates do
not clear it. No browsing history format or persistent preference changes.

The destination follows the ComboBox's user activation signal, including popup
confirmation, rather than its initialization or model binding updates.
The visible selector measures its widest destination label and caps itself to
the row width. Buttons wrap to another line when necessary. Hidden selectors
in other library kinds retain the default content-width policy.

## Verification

The focused library suite passed **51 Qt Test checks**, with no failures or
skipped checks. New coverage includes both targets through list Enter, popup
confirmation, real delegate recycling, isolation between nodes, metadata
updates, catalog replacement and returning to Current tab. The existing Reopen
button check also uses a real keyboard destination selection.
All three destination labels fit at both large text sizes, and the journey
action buttons remain within the narrow viewport.

The native `journeyDestination` cases visit two owned loopback pages, choose
New tab or New window using the entry controls, return to list browsing and
press Enter. Assertions check the actual WebEngine URL, tab/window counts and
preservation of the original page. The fixture uses 20-point text in a narrow
window. Seed command submission uses Qt events. List navigation, destination
selection and reopening use the compositor seat in the Wayland gate and Qt
events offscreen. The fixture waits for the initial native page to present a
frame and reads each rendered document's `location.href` before leaving it.
It also waits for each seed URL's real `journey_nodes` record through a read-only
connection to its own temporary SQLite profile before navigating away. This
makes the reopen assertion start with two saved visits.
Reopened tabs and windows receive the same document URL check. URL setters and
the `loading` flag alone can precede the document; load events remain available
as failure diagnostics.

The final release browser and engine build passed in **7m53s** on Qt/WebEngine
6.11.2. The complete QML suite passed **823 checks**, with no failures or skips,
in 132.117 seconds. Its log has no runtime type, reference, binding-loop or
assignment errors. Final native qualification finished on 2026-10-21:

| Gate | Qt Test checks | Result |
| --- | ---: | --- |
| `application_switcher` | 33 | Passed |
| `application_command_editing_wayland` | 12 | Passed |
| `application_keyboard_chrome_wayland` | 15 | Passed |

These gates passed serially in **185.03 seconds**. Each compared seven compiled
chrome resources against the current source. Both destination cases passed
offscreen and through native compositor key delivery, with no source overrides
or input retries. The final targeted compositor run also passed all four checks
including setup/cleanup in 4.867 seconds. Its 390×700, 20-point screenshots were
inspected: New tab and New window are fully readable and adjacent buttons wrap
within the viewport.

The separately rebuilt application executable passed `productPolishLibrary`
and `productPolishPrivateLibrary`: four checks including setup/cleanup in
9.714 seconds. These cover real storage save failure/retry, deletion, quickmark
editing, background reads and private-history transfers. Together with the
three keyboard gates, the final run passed **64 native checks**, with no
failures or skips. Interim diagnostics and failed readiness probes are not
counted as successful qualification.

Lint reports no critical/error/fatal diagnostics. Python compilation of the
fixture runner and the final diff whitespace check passed.

```sh
ctest --test-dir target/application-tests --parallel 1 -R '^application_(switcher|keyboard_chrome_wayland|command_editing_wayland)$' --output-on-failure
env QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software QT_QPA_PLATFORMTHEME= QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu target/application-tests/application_tests productPolishLibrary productPolishPrivateLibrary
```

Qualification uses software rendering, temporary profiles and an owned Wayland
compositor. It covers compositor-to-client key delivery; physical keyboards,
desktop compositor integration, platform IMEs and human accessibility review
remain outside these gates.
