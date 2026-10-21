# Downloads keyboard usability — 2026-10-21

## Reproduction

The download list ignored j/k, page and endpoint navigation. Tab could not
enter the selected download's actions. Details omitted the full filename and
destination even though both overview labels could elide them. Expanding a
row and scrolling far enough to recycle its widget lost the disclosure.
Completing a transfer left keyboard focus on its hidden Pause control.
At 390 pixels/20-point and 320 pixels/26-point, fixed toolbar buttons extended
beyond the panel. Eight focused checks failed before the repair.

## Repair

Downloads uses the shared manager list and rendered-row paging. Arrows and
j/k select downloads; Page Up/Down preserve reading overlap, including within
long details. Home/End and g/G reach endpoints. Enter toggles details, without
opening a file. Tab enters the selected row's first available action;
Shift+Tab returns to browsing. The toolbar is reachable from browsing by
Shift+Tab. The selected row has a visible accent border.

Toolbar and row buttons wrap inside their available width. The toolbar and
empty explanation scroll with the list. Minimum reading space follows the
font height. Full filenames and locations appear as wrapping plain text in
Details. Disclosure state belongs to the download ID and survives delegate
recycling and row movement; closing or changing the owner clears it.

Focus tracks the action the user is operating. If completion, retry or
preparation makes it unavailable, focus moves to an available row action or
the toolbar's Close button. Progress updates preserve focus after the user
moves elsewhere. Focusing a control reveals it inside the viewport.

## Verification

The focused Downloads suite passed **18 checks**, including toolbar traversal,
disclosure identity, available-action recovery and protection against focus
stealing. The workspace suite passed **11 checks**, and the existing product
polish suite passed **22 checks**. The 390-pixel/20-point and 320-pixel/26-point
renders were inspected. Lint reports 84 warnings and no error/critical/fatal
diagnostics.

The final focused suite also reads a failure explanation taller than several
viewports. It requires each page to advance with overlap while retaining that
download's selection, then proceeds to the next download only after reaching
the explanation's end. It checks g/G endpoints afterward.

A source-QML native probe passed both primary and secondary paging journeys
through the owned Wayland seat. A separate source probe passed keyboard
pause/resume through actual loopback transfers in normal and private windows,
completion focus, cancellation, retry and private-window ownership. These
source probes are exploratory checks; they do not substitute for rebuilding
and testing the compiled production resources.

The final full QML run passed **887 checks**, with no failures or skips in
179.763 seconds. Its log contains no runtime type, reference, assignment or
binding-loop errors. The earlier full run found an empty-footer height
binding warning despite passing its assertions; an independent footer item
removed that dependency, and the workspace suite now fails on such warnings.

The initial production release build passed in **8m 31s**.

The first compiled-resource qualification passed **14 navigation/history**,
**38 offscreen keyboard**, **12 Wayland editing** and **20 compositor-seat
keyboard checks**, with no failures or skips. The guard compares fifteen
embedded UI resources against their source files, including Downloads and
its workspace. The Wayland gate sends fresh navigation/activation keys from
an owned seat and verifies one native press/release pair and its modifiers.

The compiled real-download run passed **38 checks** in 85.329 seconds. It
covers page-preserving initiation, redirects, cookies, normal/private keyboard
pause/resume, completion focus, cancellation, retry, peer-profile controls,
owner retirement, live progress, history write contention and script-policy
ownership. No runtime type, reference, assignment or binding-loop errors
appear in the keyboard or download qualification logs.

Visual inspection after those gates found a further defect: returning from
the last expanded/collapsed download to the toolbar could leave the list blank
below it. Two added native assertions failed, one in each window. Diagnostics
showed a header at y=-4614 with the real list origin at y=6; focus reveal
scrolled to that stale estimate and never materialized the first row. A plain
force-layout call did not repair it. Materializing and polishing the first
row before moving focus to the toolbar anchors its position correctly.
Both source-native journeys then passed, with rows visible beneath the toolbar.
The selected download is retained. The QML suite also exercises the full
page/end/disclosure/toolbar sequence.

The final release rebuild, including that toolbar repair, passed in **8m 02s**.
The rebuilt Downloads journeys passed in both offscreen and owned-seat Wayland
windows with the new visible-row and preserved-selection assertions. Both
Wayland renders were inspected and show the first download beneath the toolbar.

The broad rerun also exposed a setup race in the existing combined-limit
fixture: it submitted its temporary setting during a configuration reload and
received the explicit retry response. The fixture now retries only that exact
busy response for a bounded five seconds before applying its original limit
and ownership assertions. Other rejections still fail the test.
The repaired fixture's complete offscreen gate passed **38 checks** in
126.453 seconds, with no failures or skips. The final release's other native
gates passed **14 navigation/history**, **12 Wayland editing** and **20
compositor-seat keyboard checks**. Final screenshot qualification reads the
actual committed client buffer through the owned compositor and waits for
painted title and filename pixels. Its four-check primary/secondary run
passed in 9.361 seconds. The complete frames were inspected; a misleading
PNG preview was cross-checked against pixel counts and a JPEG review copy.

The final real-download run passed **38 checks** in 84.615 seconds. The final
manager, regular/private library, new-window, popup, session-preview and
startup-confirmation run passed **21 checks** in 70.967 seconds. Together with
the four native gates, final qualification covers **143 native checks** with
no remaining failures or skips. The final logs contain no runtime QML type,
reference, assignment or binding-loop errors. No production UI changes were
made after the final release build.

Qualification logs and renders are retained in
`/tmp/ferric-download-keyboard-20261021/`. Source QML checks, compiled-resource
native checks and the release build are recorded separately. The Wayland
fixture owns its compositor, keyboard seat, D-Bus session, profiles and files;
it does not operate the user's desktop or downloads.
