# Persistent private and temporary window identity

Status: compiled offscreen, owned Wayland, rendered and unit qualification passed, 2026-10-23.

## Reproduction

The starting browser sources match the completed 55-file search-direction
revision. The new native matrix uses actual regular, private and temporary
profiles in primary, secondary and popup windows. With tabs and status chrome
hidden, all six private/temporary cases lack an explicit title marker and a
persistent visible identity; all three regular cases pass their checks.
`target/privacy-identity-baseline-native.log` records five passes including
setup/cleanup, six failures, and 32.58 s runtime.

The baseline captures all nine 360-by-640 windows at 26-point chrome text in
`target/privacy-identity-baseline-images`. The inspected primary-private image
has only webpage content, with no visible indication of its window type.

## Change

A shared noninteractive header identifies Private or Temporary windows outside
the webpage. It stays present with every status-bar policy, during command and
search entry, and with tabs hidden or placed on any edge. Its font follows the
owning window and fits constrained widths. It never takes keyboard focus.
Regular windows retain a zero-height, hidden identity header.

Primary and secondary titles include the window type. Popup titles put that
marker before page-controlled text and keep it when the page title changes.
The primary title also uses the singular "tab" for a single tab.
The secondary window's permanently hidden legacy address/navigation toolbar
is replaced by the shared identity header; command and page-search surfaces
remain the actual editors.

The shared owned-seat test driver no longer renames the application window,
which used to replace its QML title binding. It resolves one mapped window
using the application's real title and PID, and fails on ambiguity. It does
not repair page focus or replace the title before sending a gesture.

## Verification

All 12 focused header checks pass, covering regular/private/temporary states,
explicit accessible names, keyboard focus, and 320-pixel views at 26- and
60-point text: `target/privacy-identity-focused-qml.log`.
Two of those cases require nonzero intrinsic height even under a hidden parent,
so a host can use that height to decide whether to show its header. The earlier
visibility-based calculation fails both cases in
`target/privacy-identity-intrinsic-baseline-qml.log`; label-based height passes.
The final complete QML suite passes 1,187 checks in 214.32 s with forced GC:
`target/privacy-identity-final-full-qml.log`.

The header revision is frozen in `target/privacy-identity-source-hashes.json`
(57 files); its all-target release build passes in 14 min 11 s. The first native
run exposes an asynchronous fixture resize measurement and a pre-existing popup
caption issue. The corrected fixture waits for Chromium's dimensions to match
the actual view before capturing its normal-mode baseline.

A focused popup probe shows both document.title and the native view title as
Changed page title while the projected load title and window caption retain
the original URL: `target/privacy-identity-caption-baseline-native.log` (two
setup/cleanup passes, one failure, 22.26 s). Popup captions now use the live view
title for web pages and the owned caption for native pages. Both pass through
the existing bounded title filter, exposed as a read-only Qt method. Native
checks also require bidi-control filtering and working keyboard input after
the title changes, with the privacy prefix retained.

The final revision is frozen in
`target/privacy-identity-final-source-hashes.json` (57 files). Its only changes
from the built header revision are the popup caption binding and that pure
filter bridge. Relative to the preceding search-direction pass, production
changes are the two browser hosts, popup captions, the shared header and the
two Rust bridge/implementation files.

The native matrix checks the actual engine profile's off-the-record state,
visible identity and title, font scaling, three status policies, command/search
entry and cancellation, stable webpage dimensions during those transitions,
page focus and keyboard scrolling, all four tab edges, and popup title changes.
It captures the initial narrow-window result for visual review and rejects
stale embedded header and window resources.

The offscreen and owned Wayland identity gates run the same nine cases.
Search, Guide, bottom-chrome layout and Wayland recovery checks qualify the
shared host changes and title-preserving key driver. The exact release engine
and CLI unit executables emitted by the build also qualify this revision.

The first surviving Wayland identity run rejects the physical AC10 position
used for colon, before command entry, in all nine cases. The owned compositor
previously registered only AB10 (slash). It now resolves both positions from
its actual US XKB keymap, preserving native delivery rather than substituting
Qt key events. The failed diagnostic is
`target/privacy-identity-final-application_privacy_identity_wayland-before-keymap.log`.

## Compiled and rendered results

The final all-target release build reports success in 14 min 01 s. The original
long-running wrapper was subsequently terminated by SIGTERM before returning a
Wayland result; no fixture processes remained. Remaining gates resumed with
isolated process groups. No cause is claimed for that wrapper interruption.

All planned native gates pass against the frozen production sources:

| Gate | QtTest passes | CTest time |
| --- | ---: | ---: |
| Identity, offscreen | 11 | 89.15 s |
| Identity, owned Wayland | 11 | 42.72 s |
| Page search, offscreen | 38 | 113.14 s |
| Page search, owned Wayland | 32 | 86.52 s |
| Keyboard Guide, offscreen | 6 | 18.11 s |
| Keyboard Guide, owned Wayland | 6 | 13.68 s |
| Bottom chrome, owned Wayland | 11 | 40.84 s |
| Navigation recovery, owned Wayland | 6 | 23.28 s |

Detailed results are `target/privacy-identity-final-application_*-qt.log`.
The direct offscreen bottom-layout check passes 11 checks in 39.25 s; popup
commands pass three in 5.28 s. All 111 release CLI tests pass in 5.28 s.

All six private/temporary search-entry images in
`target/privacy-identity-final-native-images` were inspected. The 360-by-640
views show the identity header, native match highlighting, complete match
feedback and search input together at 26-point chrome text. The directory
contains normal and search captures for all nine host/profile combinations.

The engine unit run identified three obsolete source-layout expectations for
the hidden toolbar and earlier title expression. They now check the live
sanitized status bindings and rely on the native identity/layout gates for host
geometry; the off-the-record setup checks remain. This changes test code only.
All 57 production hashes remain unchanged. The final all-target rebuild passes
in 13 min 06 s. Its exact engine unit executable passes 502 tests in 0.38 s,
with one opt-in benchmark ignored. The harness is then relinked against that
archive and all nine identity cases pass again (11 QtTest checks, 49.99 s).
Logs are `target/privacy-identity-final-unit-build.jsonl`,
`target/privacy-identity-final-ferric_browser_engine_qt-units.log` and
`target/privacy-identity-final-final-native-identity.log`.

`target/privacy-identity-final-qualification-status.json` records complete
success after the source checks, native gates, exact release unit executables
and final relinked identity run. Requirement statuses remain in progress for
the broader profile, desktop and accessibility contracts.
