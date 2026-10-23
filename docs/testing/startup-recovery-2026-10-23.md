# Startup recovery preserves current work and lazy addresses

Status: qualified against the canonical release archive and CLI, 2026-10-23.

## Reproduction

The previous qualified engine fails all four new startup scenarios, with two
setup/cleanup passes and four failures in 67.03 seconds
(`target/startup-recovery-baseline-native.log`). They cover an initial native
start page, one incoming HTTP page, several incoming tabs with recent closed
work, and another tab arriving while the saved-file read is held. The fixture
reopens the profile before recovery and deliberately flushes current work
before accepting it. This replaces the earlier warm-profile shortcut with the
actual source-selection boundary.

The public-CLI restart test also fails on the unchanged CLI executable. After a
real forced exit and native recovery, `session-save after-recovery` records the
unvisited old-A address as `about:blank` (5.93 seconds;
`target/startup-recovery-restart-baseline.log`). The earlier independent probes
also establish overwritten incoming work, current-run checkpoint duplication
and model/native URL disagreement; their evidence remains in
`docs/testing/recovery-dialog-2026-10-23.md`. Baseline artifact hashes are in
`target/startup-recovery-baseline-artifact-hashes.json`.

## Change

The profile worker discovers recovery paths before the new owner writes a
checkpoint. The recovery banner does not pause ordinary checkpoint writes;
its bootstrap guard only skips clearing the old files. Same-profile windows
share the first owner's source set. Recovery reads those paths and never rescans
the live directory. Retry retains the same
source authority; cancellation and profile replacement still reject stale
read completions through their storage tickets.

Recovery evaluates current work when its read completes. It appends recovered
tabs while preserving current renderers, document identities and drafts. Only
the captured initial blank/start-page target can be reused, and only while its
owner, target, sole-tab membership, default settings and empty live Undo list
still match. Appending preserves navigation/document bookkeeping and merges
closed descriptors without replacing current Undo. A closed-tab-only recovery
keeps the current page and recovers its Undo records.

A reducer event retains a new idle tab's restored address without loading a
document, advancing its target or creating a history visit. It cannot overwrite
an existing address or an active navigation. Persistence and tab queries now
have the same authoritative address that the lazy QML row displays.

Format 4 records `recovered_session_ids`. A saved merged checkpoint covers its
ancestor files, so another crash does not recover those files twice. Older
files remain available if the current generation is corrupt and atomic loading
falls back to a generation that had not incorporated them. The worker selects
the newest valid revision of each session, rejects ancestry cycles, and merges
bounded closed history. Formats 2 and 3 upgrade in memory without rewriting the
source; invalid self/duplicate identities cannot replace a valid generation.
Normal checkpoint clearing removes automatic current and previous generations,
including the legacy `current` file, while preserving named sessions.

Opening the recovery banner captures its focus return point. Accepting or
dismissing recovery closes that overlay through the normal focus controller,
so keyboard input returns to the current page or native surface.

## Verification

The native scenarios use production QML, the storage worker and owned keyboard
input. Their Wayland counterpart observes physical-seat delivery. A controlled
read barrier allows a new tab to arrive before completion. They require current
Core IDs, QObject identities and form values to survive, and verify saved
pin/mute/zoom before visiting the lazy recovered page. Current and recovered
closed work must both reopen through `u`.

The external-process gate uses two independent disposable profiles: startup at
the native start page, and startup with two real CLI-created HTTP pages. Each
has two forced-exit/recovery cycles. It checks exact model addresses, preserved
current IDs, no repeated GET for current work, no GET for unvisited old-A, the
named snapshot, and a merged current checkpoint before the second exit. The
test-only native input plugin requires an owned marker and offscreen platform;
the D-Bus configuration cannot activate installed services.

An additional owned process probe covers a snapshot containing only closed
tabs. It keeps both current pages, retains recovered closed-tab settings in a
named snapshot and restores the closed page through public `tab-undo` after a
second crash. The probe passes against the final CLI executable; its log is
`target/startup-recovery-closed-only-final.log`. This target-local diagnostic is
separate from the committed two-scenario restart gate and native Qt totals.

The first release attempt stopped at two omitted exhaustive event-projection
matches. The final revision treats restored addresses as tab metadata and
captures the target without exposing the URL in notification payloads. A stable
closed-history sort also preserves same-second Undo ordering; all 110 storage
checks pass with that regression assertion. Earlier build diagnostics and
manifest hashes are retained under `target/startup-recovery-pre-projection-`.

The second release passed all 1,028 selected Rust unit checks and the new
offscreen, owned Wayland and real-process startup gates. Its cold-start dialog
gate then exposed focus returning to a hidden button after dismissal. The
production focus handling above corrects this; a local-QML diagnostic passes
all 10 dialog checks, but is not counted as embedded release qualification.
That revision also failed strict Clippy because profile bootstrap exceeded its
function-length limit; importing the source-discovery helper resolves it.
Diagnostics and frozen inputs remain under `target/startup-recovery-pre-focus-`.

The next release passes the embedded cold-start dialog gates offscreen and on
Wayland. Two compatibility fixtures initially install their old generation
while the warm owner can still write. Installing it after teardown and before
cold bootstrap passes both current-format and version-2 cases (four checks
including setup/cleanup). The final revision changes that fixture boundary and
three Rust test-style issues only. The separate Qt/CLI strict Clippy check
passes; the full final command remains part of qualification below. Earlier
logs and manifests remain under `target/startup-recovery-pre-test-cleanup-`.

| Check | Result |
| --- | --- |
| Selected release Rust unit checks | 1,028 passed; one optional Qt benchmark ignored |
| Full QML suite | 1,202 passed (228.87 s) |
| Native Qt checks | 278 passed across the gates below |
| Real-process restart scenarios | 2 passed; two forced exits and recoveries per scenario |
| Additional closed-tab-only process probe | 1 passed; two forced exits and recoveries; excluded from native and CI gate totals |
| Strict Clippy | Six packages, all targets, warnings denied (19.47 s) |
| Formatting and whitespace | cargo fmt --all -- --check; git diff --check passed |
| CI selection | All three new gates selected exactly once by the actual CI regex |

| Native gate | Passed | Seconds |
| --- | ---: | ---: |
| `application_startup_recovery` | 6 | 16.70 |
| `application_startup_recovery_wayland` | 6 | 16.70 |
| `application_session_recovery` | 10 | 24.32 |
| `application_session_recovery_wayland` | 10 | 24.37 |
| `application_tab_recovery` | 44 | 125.19 |
| `application_tab_recovery_wayland` | 44 | 122.77 |
| `application_close_requests` | 16 | 24.70 |
| `application_switcher` | 108 | 280.04 |
| `application_tab_focus_wayland` | 26 | 55.61 |
| `session-consumers` | 5 | 9.05 |
| `resource-parity` | 3 | 1.79 |

The native binaries link `target/release/libferric_browser_engine_qt.a`.
Resource parity checks the embedded QML against the checkout. The source,
fixture and CI manifests cover 73, 17 and 1 unchanged files.
The local-QML focus diagnostic is excluded from these totals. The two
process scenarios are counted separately from the 278 native Qt checks.
Hosted CI was not run; its configured gate selection was checked locally.
Final results and artifact hashes are in
`target/startup-recovery-final-facts.json` and
`target/startup-recovery-artifact-hashes.json`.

Frozen inputs are recorded in
`target/startup-recovery-{source,fixture,ci}-hashes.json`; release and runner
status is in `target/startup-recovery-progress.json` and
`target/startup-recovery-final-status.json`.

## Remaining scope

Named-session replacement still uses its existing preview and unsaved-page
policy. Workspace/geometry recreation, real filesystem-failure behavior and
arbitrary manual checkpoint edits are not qualified by this pass. The separate
public CLI `open --target tab` forwarding error and closed-stdout panic remain
recorded in `target/browser-usability-follow-ups.json`. The broad browser
usability goal remains active.
