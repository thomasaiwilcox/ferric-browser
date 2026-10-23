# Stop and retained loading state

Status: compiled offscreen, owned-Wayland, resource, release-unit, QML and lint qualification passed, 2026-10-23.

## Reproduction

The starting production revision is the completed Switcher live-update pass.
A native WebEngine probe sends the configured `s` binding on a finished page,
then reselects its tab through Alt+1. In primary, secondary, private and popup
hosts, both retained and displayed state become cancelled although the native
engine emits no loading event. All four behavior rows fail in
`target/idle-stop-baseline-native.log` (two setup/cleanup passes, four failures,
10.46 s).

The extended matrix covers completed and failed pages, page-initiated GET and
POST requests before headers, committed responses whose tails are withheld,
and a fresh browser Open immediately followed by Stop. It captures actual
`QWebEngineLoadingInfo` notifications and retained tab-transfer state.
Twenty-four behavior rows fail across the four hosts; only the four queued
Open cases pass (`target/stop-state-baseline-native.log`, six passes including
setup/cleanup, 24 failures, 75.17 s).

Stop overwrites complete and failed states with cancelled. Separately,
page-initiated requests retain complete in Core while the Qt property displays
provisional. Native GET and POST starts and their subsequent stopped events
are present in the probe, so the retained discrepancy is independent of the
renderer having started or stopped its request.

## Change

A valid Stop request still reaches the native engine. It changes retained
loading only when that state is provisional or committed; idle, completed,
failed and already-cancelled pages keep their state. Forwarding remains
necessary when native loading has begun before its start callback arrives.

A target-bound `NavigationStarted` engine fact records provisional loading for
page-initiated starts through the active, indexed and popup lifecycle paths.
It emits no navigation effect, advances no generation or document identity,
and changes no URL, title or Journey entry. Browser-requested navigation keeps
its existing target invalidation and engine effect. Native start observation
and IPC projection retain the captured target without adding page content.
Stale or retired targets are rejected.

## Verification

The frozen manifests are `target/stop-state-source-hashes.json` (60 production
and embedded-test files) and `target/stop-state-fixture-hashes.json` (10 fixture
files). Seven Rust files differ from the previous production manifest; QML
production sources are unchanged.

Both Stop CTest entries are included in CI's browser-surface selection. The
actual filter selects each exactly once in a local CTest inventory check
(`target/stop-state-ci-selection.json`); the workflow has a separate hash in
`target/stop-state-ci-hashes.json` because its selection changes no compiled
input. The hosted CI workflow itself was not run here.

All 169 Core and nine runtime tests pass before native qualification. Added
regressions check all six loading states, unconditional forwarding of valid
Stop, native starts preserving tab metadata/history/target, and rejection of
stale generations, documents and closed tabs.

The final native matrix additionally reselects each active load before Stop,
requiring retained provisional state and native loading to survive. After Stop
it checks matching retained and visible state, target identity, page address,
page markers, focus, single GET/POST admission and no unexpected failure dialog.
Settled pages must produce no new native loading event. Queued Open must still
stop with its captured target.

Both Stop matrices pass against the final archive: 30 Qt checks each, including
setup/cleanup, in 147.96 s offscreen and 153.12 s through the owned Wayland seat.
The final all-target release build passes in 16 min 29 s. Its exact emitted
unit executables pass Core 169, runtime nine, engine 504 and CLI 111 tests
(793 total); one opt-in engine performance benchmark is ignored. The full QML
suite passes 1,197 checks with forced GC in 230.94 s.

Strict Clippy for all four changed/dependent crates and all targets passes with
`-D warnings` in 6 min 22.46 s; formatting checks pass.

All compiled gates pass against the frozen revision. Counts include Qt setup
and cleanup:

| Gate | Qt passes | Qt runtime |
| --- | ---: | ---: |
| Stop, offscreen | 30 | 147.96 s |
| Stop, owned Wayland | 30 | 153.12 s |
| Form reload, offscreen | 46 | 148.97 s |
| Form reload, owned Wayland | 46 | 144.66 s |
| Streaming history, offscreen | 66 | 200.89 s |
| Streaming history, owned Wayland | 66 | 198.17 s |
| Recovery, offscreen | 7 | 24.42 s |
| Recovery, owned Wayland | 6 | 23.29 s |
| Live titles, owned Wayland | 6 | 13.74 s |
| Switcher navigation, owned Wayland | 10 | 28.95 s |
| Switcher live updates, owned Wayland | 7 | 18.95 s |
| Embedded-resource parity | 3 | 1.92 s |

The 323 native Qt checks have no failures or skips. Resource bytes and
manifest hashes match all 115 embedded source files. The final audit verifies
production/fixture/CI hashes, actual gate totals and the exact release artifacts;
see `target/stop-state-final-audit.log`, `target/stop-state-final-facts.json` and
`target/stop-state-artifact-hashes.json`. Documentation generation and whitespace
checks pass. The CI selection check adds no claim of a hosted CI run.

The earlier intermittent cancelled state after private POST Back remains
unresolved. This correction follows a separate reproducible Stop request and
native-start discrepancy; it does not establish the cause of that observation.
See [the navigation evidence](navigation-recovery-2026-10-22.md#v6-compiled-verification).
