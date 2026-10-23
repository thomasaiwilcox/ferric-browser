# Page search direction continuity

Status: compiled offscreen and owned Wayland qualification passed, 2026-10-23.

## Reproduction

The starting production source matches the 45-file qualified search-polish
revision. New native checks start both forward and backward searches in
primary, secondary, private and popup windows. The qualified baseline fails
all eight rows: a reverse step overwrites a forward query's saved direction,
and n moves forward after a backward query. See
`target/search-direction-baseline-final-native.log` (two setup/cleanup passes,
eight failures, 85.20 s).

The first exploratory fixture assumed a backward query always starts at the
last match. Chromium chooses its starting position from native find/selection
state. The qualified fixture captures the actual initial position, checks it
is within the three matches, and verifies subsequent relative movement and
wrapping. It does not replace native selection or repair focus after actions.

Two focused unit baselines independently reproduce overwritten query direction
and the CLI encoder adding an unrequested forward direction:
`target/search-direction-baseline-core.log` and
`target/search-direction-baseline-ipc.log`.

## Behavior

The retained query owns its original direction. n continues that direction;
N steps in the opposite direction without changing the next n. Previous/next
buttons also apply only to their individual steps, so editing the query after
using an arrow keeps the original / or ? direction. Starting a new query can
choose a new direction.

`search-next` continues the original direction. Explicit `--forward` and
`--backward` select the direction for that invocation and combine with the
existing bounded `--count`. Duplicate or conflicting direction flags are
rejected. Typed forward actions and IPC commands retain their explicit choice;
a CLI command without a direction omits that field instead of inserting one.
The Guide shows the appropriate command equivalent for N in either direction.
Command completion offers executable flags and suppresses already chosen
options; it does not offer flags where a count value is required.

## Verification

All 164 core and 36 IPC unit tests pass, including relative/default direction,
explicit forward steps, conflicting flags, typed round trips and completion.
Strict Clippy checks pass for both crates and all targets. Logs:
`target/search-direction-core-ipc-tests.log` and
`target/search-direction-core-ipc-clippy.log`.

The complete QML suite passes 1,175 checks in 204.06 s with the required
forced-GC flags: `target/search-direction-full-qml.log`.

The final production revision is frozen in
`target/search-direction-source-hashes.json` (55 files). The native matrix also
checks seven successive n/N steps, wrapping, default and explicit counted
commands, a forward action, the Guide's N equivalent, unchanged URL and page
focus, a fresh query in the opposite direction, both arrow buttons and return
to editor input. It compares embedded search, runtime, focus and window QML
with the actual source.

The offscreen and owned Wayland-seat search gates include all eight new rows.
Keyboard gestures in the Wayland matrix come from its owned compositor seat;
button clicks are explicit Qt pointer events. The original match feedback,
composition, result activation and viewport checks remain in those gates.
The keyboard Guide gates and the release engine/CLI unit executables also
qualify the frozen revision.

The all-target release build passes in 16 min 04 s. The compiled direction
canary passes all eight data rows (10 QtTest checks including setup/cleanup)
in 59.59 s. The expanded offscreen search gate passes 38 checks in 146.22 s
CTest time; the owned Wayland gate passes 32 checks in 87.45 s. Logs:
`target/search-direction-native-canary.log`,
`target/search-direction-application_page_search-qt.log` and
`target/search-direction-application_page_search_wayland-qt.log`.

Regenerating the four registry-owned user documents produces the same content
as the updated files. Broader requirement statuses remain in progress.

Both keyboard Guide gates pass all six checks: 18.14 s offscreen and
13.56 s Wayland. All 502 release engine tests and all 111 release CLI unit tests
pass, with the engine's opt-in benchmark ignored.
`target/search-direction-qualification-status.json` records complete success
after verifying all 55 source hashes, the successful full release build,
the native gates and the exact unit executables emitted by that build.
