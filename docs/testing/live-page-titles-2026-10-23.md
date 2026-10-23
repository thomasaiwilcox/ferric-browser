# Live page titles in keyboard workflows

Status: compiled offscreen, owned Wayland and unit qualification passed, 2026-10-23.

## Reproduction

The starting production sources match the completed 57-file privacy identity
revision. After JavaScript changes `document.title` to `Live Northstar Inbox`,
the native WebEngine view reports that title, while retained `page_title` and
`yank title` still report the load-time title. A tab-scoped Switcher query for
`northstar` has no result. The corrected native baseline reproduces this in
primary, secondary, private and popup hosts:
`target/live-titles-baseline-native-2.log` (two setup/cleanup passes, four
failures, 18.19 s).

The first test draft called a nonexistent target-inspection method and aborted
on the fixture's QML error policy. That fixture error is recorded separately in
`target/live-titles-baseline-native.log`; the corrected reproduction uses
`navigation_target_for_index` and completes all four data rows.

## Change

Settled WebEngine title notifications publish a typed `PageTitleChanged` fact
through the runtime. The adapter checks the captured tab, generation and
document against its current owned target and applies the existing 4 KiB,
control/bidi-filtered title policy before reduction. Retained title changes
advance the revision used by Switcher caches and session checkpoints. Repeating
the same sanitized title creates no metadata change. Background updates leave
the active page title alone; selecting that tab reads its current retained title.

The QML tab label uses the same filter. An open Switcher coalesces title refreshes
through its existing timer, preserving the query, invocation owner and editor
focus. A transferred live view captures its admitted target without reloading
the document. Native-page titles remain owned by Ferric.

Title changes do not commit a navigation, record a visit, or change the native
history cursor. IPC publishes `tab.changed` with target identity and
`title_changed: true`; the notification does not include the page title.

## Verification

The final production and embedded-test source snapshot is
`target/live-titles-source-hashes.json` (58 files). Relative to the privacy
pass, production changes are the core reducer, runtime observation, Qt event
projection, Rust bridge and tab-state adapter, and the shared WebEngine view
inside the primary host component.
Eight native fixture sources are separately frozen in
`target/live-titles-fixture-hashes.json`.

Release core/runtime tests pass: 166 core and eight runtime tests, including
no-op metadata, title clearing, unchanged tab/window/navigation/search/Journey
state, and stale replaced/closed targets (`target/live-titles-core-runtime.log`).
An engine unit check additionally verifies that title notifications identify
their observed tab/window without exposing page text.

The native matrix seeds a cached query before changing the title, verifies
retained metadata and `yank title`, drives the real Shift+T Switcher shortcut,
and changes a matching title and then removes the match with the Switcher open.
It checks stable query and native focus, Escape returning to the page, 4 KiB
Unicode/control filtering, unchanged URL/target/history, background updates and
tab reselection. The primary case also detaches the live view, proves a
JavaScript marker survives, updates its title under the destination owner and
rejects the retired source callback. The Wayland run uses the owned compositor
seat and observes native press/release identity without replacing window titles.

The all-target release build passes in 16 min 50.71 s. Native harnesses link
against `target/release/libferric_browser_engine_qt.a`; resource parity checks
reject a stale shared WebEngine host. The exact engine and CLI unit executables
emitted by that build pass 503 and 111 tests respectively. One opt-in performance
benchmark remains ignored. With the 166 core and eight runtime tests above,
788 Rust tests pass. Strict Clippy with `-D warnings` and formatting checks pass.

The final QML suite passes all 1,187 checks in 148.92 s with forced GC. All native
gates pass against the frozen revision:

| Gate | Qt passes | Qt runtime |
| --- | ---: | ---: |
| Live titles, offscreen | 6 | 15.02 s |
| Live titles, owned Wayland | 6 | 14.24 s |
| Full Switcher, offscreen | 108 | 273.76 s |
| Privacy identity, owned Wayland | 11 | 42.40 s |
| Page search, owned Wayland | 32 | 88.44 s |
| Tab focus, owned Wayland | 26 | 54.13 s |
| Navigation recovery, owned Wayland | 6 | 23.30 s |

The six live-title passes include four host scenarios plus setup and cleanup.
The seven native runs total 195 Qt passes with no failures. Their logs, build
diagnostics, unit results, full QML output and completion checkpoint use the
`target/live-titles-final-` prefix. The strict lint and core/runtime logs use
`target/live-titles-clippy-2.log` and `target/live-titles-core-runtime.log`.
`target/audit-live-titles-final.py` checks frozen sources and fixtures, canonical
linkage, all results, requirement status and CSV structure; it records hashes
for the release library, browser and native harnesses in
`target/live-titles-final-artifact-hashes.json`.

The broad Switcher search, privacy and lifecycle requirements remain in progress;
this pass qualifies live title consistency, not every requirement in those areas.
