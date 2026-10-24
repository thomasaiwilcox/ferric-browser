# Switcher responsiveness during continuous page activity

Status: compiled offscreen, owned Wayland, embedded-resource and unit qualification passed, 2026-10-23.

## Reproduction

The starting production sources match the completed 59-file navigation-freshness
revision. A native test opens Shift+T, finds a page, starts real JavaScript title
updates every 10 ms, then replaces the query through Ctrl+A and normal keys.
The private window receives 70 native title notifications during a 703 ms check
but never publishes the new query's result. The other three hosts take 320–611 ms
to publish it. Focus remains on the same input in every case:
`target/switcher-live-updates-baseline-native.log` (five passes including setup
and cleanup, one failure, 15.81 s).

An extended matrix adds focused result actions and eight regular windows. It
again fails the private query after 71 notifications in 705 ms, while successful
queries take 270–514 ms and subsequent label updates take 515–528 ms. The final
window's result is checked while the invoking page stays busy:
`target/switcher-live-updates-baseline-native-2.log` (six passes including setup
and cleanup, one failure, 24.70 s).

Live notifications restart the same timer used to coalesce query edits. A
continuous stream can keep moving its deadline. Merely starting an idle timer
also proves insufficient: two focused tests fail because frequent refreshes
keep cancelling the later sources in a twelve-window batch
(`target/switcher-live-updates-throttle-only-qml.log`, four passes, two failures).

A further stress test schedules the first source 100 ms later. The first guard,
which depended on already-published results, cannot protect a batch that has not
published its first result. All four behavior checks fail in
`target/switcher-live-updates-slow-first-source.log` (two setup/cleanup passes,
four failures). The preliminary build and lint jobs were explicitly stopped for
this source correction; their logs use `target/switcher-live-updates-pre-slow-fix-`,
with the reason recorded in `target/switcher-live-updates-pre-slow-fix-cancelled.json`.

## Change

Typed edits retain their existing 35 ms debounce. Live changes start an idle
refresh timer without postponing pending keyboard work. When a batch for the
same captured query and scope is still querying windows, it finishes before
another live refresh starts. This protects the first source as well as later
ones. New input still replaces the batch immediately.

Storage-library completions use this same live-refresh path. Completing or
invalidating a batch replays pending live work; closing the Switcher clears it.
Existing result reconciliation retains selected identities and focused actions
while labels update.

## Verification

The final revision is frozen in `target/switcher-live-updates-source-hashes.json`
(60 production/embedded-test files). Nine fixture hashes cover eight native
sources and the new QML scheduling test. Production changes are the primary and
secondary hosts and the existing window presentation controller.

Ten focused QML checks pass in 4.07 s, including setup/cleanup: query progress,
all twelve window sources finishing, edits replacing an in-flight batch, and
close discarding pending work. Each behavior runs with sources scheduled at
20 ms and 100 ms, including the first source. See
`target/switcher-live-updates-focused-qml.log`.

The native matrix uses real WebEngine title notifications and owned key delivery.
It requires query results and further live updates within bounded checks while
the JavaScript stream is still running, preserves input and focused-action
identity, and returns focus through Escape without changing URL or document
target. The eight-window case finds the last owner's result and observes that
owner's title changing while the invoker continues updating.

The first Wayland run passes all four host types, then fails while establishing
the eight-window invoker before opening its Switcher. An untokened
`requestActivate()` does not restore compositor focus after seven other windows
have opened. The fixture now establishes initial focus through an observed
owned-seat Escape before the tested gestures; it makes no focus repairs during
the search or action checks. Production sources are unchanged. The diagnostic
is `target/switcher-live-updates-final-application_switcher_live_updates_wayland-before-seat-setup.log`
(six passes, one setup failure, 33.44 s). Both complete live-update matrices pass
after that fixture correction.

The final all-target release build passes in 18 min 2.64 s. Strict Clippy with
`-D warnings` passes in 10 min 18 s, and formatting checks pass. The exact engine
and CLI unit executables emitted by the release build pass 503 and 111 tests
respectively (614 total); one opt-in performance benchmark remains ignored.

The full QML suite passes 1,197 checks with forced GC in 288.69 s against the
frozen final sources (`target/switcher-live-updates-full-qml.log`). All native
gates pass:

| Gate | Qt passes | Qt runtime |
| --- | ---: | ---: |
| Live updates, offscreen | 7 | 20.54 s |
| Live updates, owned Wayland | 7 | 18.86 s |
| Navigation freshness, offscreen | 10 | 32.39 s |
| Navigation freshness, owned Wayland | 10 | 28.77 s |
| Live titles, offscreen | 6 | 16.40 s |
| Live titles, owned Wayland | 6 | 14.36 s |
| Full Switcher, offscreen | 108 | 279.79 s |
| Privacy identity, owned Wayland | 11 | 42.89 s |
| Page search, owned Wayland | 32 | 86.87 s |
| Tab focus, owned Wayland | 26 | 54.29 s |
| Recovery, owned Wayland | 6 | 23.31 s |
| Embedded resource parity, offscreen | 3 | 1.90 s |

The two live-update matrices each contain five scenarios plus setup/cleanup.
After typing, results are observed in 0–29 ms offscreen and 44–58 ms on the owned
Wayland seat. Zero means a result was already available at the first observation.
Further label updates arrive in 33–55 ms and 33–75 ms respectively, with the page
stream still active and the same action focused. Timings describe this fixture
run. Both environments find and update the last owner's row among eight windows.

The twelve native runs total 232 Qt passes with no failures. The resource gate
compares all 115 embedded source files byte-for-byte and checks their manifest
hashes, including the corrected presentation controller. Both harnesses link
against the canonical `target/release/libferric_browser_engine_qt.a`.

Final logs, release build diagnostics, release-unit results and the completion
checkpoint use `target/switcher-live-updates-final-`. The setup-only fixture
correction resumes through `target/resume-switcher-live-updates.py`; production
hashes remain unchanged throughout that resumption. Its final runner log is
`target/switcher-live-updates-final-resume.log`.

`target/audit-switcher-live-updates-final.py` checks the sixty frozen production
and embedded-test sources, nine fixtures, canonical linkage, all results,
requirements and CSV structure. It records hashes for the release library,
browser and native harnesses in
`target/switcher-live-updates-final-artifact-hashes.json`.

Broad scale and accessibility qualification remain in progress.
