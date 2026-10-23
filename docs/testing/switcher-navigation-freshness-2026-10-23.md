# Switcher freshness across navigation

Status: compiled offscreen, owned Wayland and unit qualification passed, 2026-10-23.

## Reproduction

The starting production revision matches the completed live-title pass
(`target/live-titles-source-hashes.json`, 58 files). An eight-scenario native
matrix opens the real Shift+T Switcher, searches for an absent destination,
then changes the page through a typed `open` command or `history.pushState`.
It checks the destination in both a repeated backend query and the already-open
Switcher, retaining the input query and native focus.

The first run fails full navigation in the private window while the other
seven host/method cases pass: `target/switcher-navigation-baseline-native.log`
(nine passes including setup/cleanup, one failure, 29.90 s). The title-less
initial document permits a URL-derived title change to invalidate the cache
after a same-document URL change. A corrected fixture sets and waits for a fixed
`Original document` title before opening the Switcher, isolating URL freshness.
It fails both private-window cases while the six regular cases pass:
`target/switcher-navigation-baseline-native-2.log` (eight passes including
setup/cleanup, two failures, 31.71 s). Native focus remains on the same editor in
every case, including failures.

The cache keys the durable state revision, but navigation facts can update
retained tab URL/title/generation without advancing that revision. Regular
profiles have independent storage-library invalidation and refresh paths;
transient profiles do not have that durable worker. The native private case
demonstrates that navigation cannot rely on those incidental updates.

## Change

The runtime observation boundary now invalidates the Switcher cache on accepted
runtime changes, then notifies the host that its live sources changed. Primary
and secondary hosts schedule the existing coalesced Switcher refresh; popups
inherit the secondary host. This follows publication of accepted core state and
does not wait for storage-library completion. Durable revision semantics and
private-source admission remain unchanged.

Live title updates use the same notification path instead of a separate
view-local refresh. Queries, invocation ownership, selected-row reconciliation
and keyboard focus continue to use the established Switcher implementation.
The fixture also navigates background tabs while the Switcher stays open and
requires the selected page and editor focus to remain unchanged.

## Verification

The frozen revision is `target/switcher-navigation-source-hashes.json` (59
production/embedded-test files), with eight native fixture sources in
`target/switcher-navigation-fixture-hashes.json`. Production changes from the
live-title pass are the Rust runtime bridge, one Qt signal declaration, and its
primary/secondary host handlers. The old view-local title refresh is removed.

The native matrix covers full navigation and same-document changes across four
hosts, cached empty searches, automatic visible refresh, current result
generation, Enter activation, restored page focus, and background navigation.
Same-document cases preserve a JavaScript marker and the captured target;
full typed navigation must replace the captured target. Offscreen and owned
Wayland gates run the same eight scenarios. The Wayland driver observes native
seat press/release identity without replacing production window titles.

The all-target release build passes in 17 min 21.59 s. Both native harnesses
link against the canonical `target/release/libferric_browser_engine_qt.a` and
reject stale embedded host resources. The exact engine and CLI unit executables
emitted by the build pass 503 and 111 tests respectively (614 total); one opt-in
performance benchmark remains ignored. Strict Clippy with `-D warnings` passes
in 9 min 30 s, and formatting checks pass.

The full QML suite passes 1,187 checks with forced GC in 253.93 s against the
same frozen sources (`target/switcher-navigation-full-qml.log`). All native
runs pass:

| Gate | Qt passes | Qt runtime |
| --- | ---: | ---: |
| Navigation freshness, offscreen | 10 | 32.67 s |
| Navigation freshness, owned Wayland | 10 | 28.59 s |
| Live titles, offscreen | 6 | 14.64 s |
| Live titles, owned Wayland | 6 | 14.09 s |
| Full Switcher, offscreen | 108 | 276.16 s |
| Privacy identity, owned Wayland | 11 | 42.45 s |
| Page search, owned Wayland | 32 | 87.52 s |
| Tab focus, owned Wayland | 26 | 53.67 s |
| Navigation recovery, owned Wayland | 6 | 23.43 s |

The navigation matrix has eight host/method scenarios plus setup/cleanup in
each environment, with background checks in its six non-popup scenarios.
The nine native gates total 215 Qt passes with no failures. Build diagnostics,
release-unit results, native logs and the successful completion checkpoint use
the `target/switcher-navigation-final-` prefix. Lint output is
`target/switcher-navigation-clippy.log`.

`target/audit-switcher-navigation-final.py` checks frozen production and fixture
hashes, canonical linkage, all results, requirement status and CSV structure,
and records hashes for the release library, browser and native harnesses in
`target/switcher-navigation-final-artifact-hashes.json`.

Broad search, scale, privacy and compositor qualification requirements remain
in progress.
