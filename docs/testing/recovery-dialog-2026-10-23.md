# Recovery dialog retry and cancellation

Status: scoped dialog, native offscreen/Wayland, resource, unit, QML and lint qualification passed, 2026-10-23.

## Reproduction

The previous qualified release archive reproduces six keyboard failures.
After a corrupt saved-session read, Escape and Alt+D cannot dismiss the dialog,
and repairing the file then pressing Alt+R cannot retry. During a blocked read,
Escape, `n` and Alt+D also cannot dismiss it. The native matrix reports two
setup/cleanup passes and six failures in 83.22 seconds; see
`target/recovery-dialog-baseline-native.log` and
`target/recovery-dialog-baseline-artifacts.json`.

`FerricCommandDialog` marks a decision taken before emitting the action. The
recovery surface previously stayed visible after read failure without resetting
that guard or exposing an error. It also reused the named-session preview
loading flag. Backend completions discarded their storage ticket and chose
recovery, load or preview from shared pending flags, so merely clearing the
recovery flag would let its late completion become a preview.

An initial fixture attempt removes the wrong backup filename and fails before
the behavior check. Ferric correctly falls back to the retained `.previous`
generation. The corrected corruption fixture removes that generation using the
storage format's actual extension, and then reproduces the six behavior failures.
That initial run is `target/recovery-failure-baseline-native.log`.

## Change

Recovery projects its own pending and error state. The surface shows progress,
a useful read/empty error and a retry choice. Restore keys are disabled during
the read; the safe choice and Escape remain available. State changes rearm the
shared dialog guard through its existing action-change behavior. Named-session
preview no longer owns recovery's loading state. Dismissal clears the stale
loading status and reports that recovery was skipped.

Each session read retains its storage ticket and typed intent: recovery, named
load with append policy, or preview. Completion consumes only that ticket's
intent. Dismissal removes recovery intents, so delayed reads cannot restore
tabs or fall through to preview. Profile replacement clears the intents and
recovery projections. The storage worker still owns the read; cancelling the
dialog revokes publication without blocking the UI on that worker.

## Fixtures and qualification

The native fixture uses the production QML dialog, storage worker and keyboard
router, with its own temporary profile and loopback page. It flushes a real
checkpoint, then enables the recovery surface to isolate read outcomes after
bootstrap. Five failed/empty scenarios check Escape, Alt+D and successful retry
after saved data is repaired or appears. Three delayed-read scenarios check
Escape, `n` and Alt+D after repeated `y` and Alt+R. A test-only interposer holds
one exact checkpoint read for at most 15 seconds, with a scope guard releasing
it on assertion failure. No production test hook or timing race controls the
read. A subsequent named preview proves the restore lane has processed the
cancelled completion. Original Core ID, renderer identity, in-document draft and
page focus must survive. The owned Wayland entry observes native key delivery.

The first compiled revision passes all five error/empty scenarios but fails
all three delayed-read checks because dismissal leaves the status bar saying
"Loading recovery session" (seven passes, three failures, 14.31 seconds;
`target/recovery-dialog-pre-status-native.log`). The final revision clears that
status. It also observes the cancelled completion before requesting a named
preview, requiring both the preview and restore payload to remain unchanged.
The earlier release build took 13m36s. The qualification guards rejected that
revision after the source refinement, before running its release units or lint.
It does not qualify the final correction.

Focused QML checks exercise disabled restore keys during loading and keyboard
retry after error: 60 passes in 7.06 seconds. The full unchanged QML suite passes
1,202 checks in 217.75 seconds.

The final release build completes in 13m32s. All 900 selected release units
pass: storage 107, Core 169, runtime 9, Qt adapter 504 and CLI 111. One optional
Qt benchmark remains ignored. Strict Clippy passes for all five packages and
all targets with `-D warnings` in 401.43 seconds. Formatting and whitespace checks pass.

| Native gate | Passed | Time |
| --- | ---: | ---: |
| Recovery dialog, offscreen | 10 | 30.90 s |
| Recovery dialog, owned Wayland | 10 | 39.35 s |
| Closed-tab/checkpoint recovery, offscreen | 44 | 236.91 s |
| Closed-tab/checkpoint recovery, owned Wayland | 44 | 133.87 s |
| Close requests | 16 | 23.75 s |
| Full switcher | 108 | 272.77 s |
| Tab focus, owned Wayland | 26 | 52.99 s |
| Secondary managers, save backpressure and preview | 5 | 9.18 s |
| Embedded resources | 3 | 1.88 s |

The native total is 266 checks, including each QtTest process's setup/cleanup.
Embedded bytes and manifest hashes match all 115 source files. The existing
session-consumer cases verify named-session replace and append, existing view
retention, save backpressure and failed previews through the revised read
routing. The new read interposer is loaded only by the two recovery-dialog
entries. Both are selected once by the actual CI browser-surface regex (25
selected tests); hosted CI itself was not run.

The final audit verifies 69 frozen source files, 14 fixture files, the CI
workflow, the canonical release archive used by CMake, exact Cargo unit
executables, all reported totals and artifact hashes. Inputs and results are
recorded by `target/recovery-dialog-{source,fixture,ci}-hashes.json`,
`target/recovery-dialog-final-facts.json`,
`target/recovery-dialog-artifact-hashes.json` and
`target/recovery-dialog-final-audit.log`. All owned build, test and lint jobs
have finished.

## Limits and follow-up

This pass does not change successful recovery's replace/append policy or source
discovery. A separate warm/cold probe against the captured earlier release
binary now confirms the deferred-recovery defect through public CLI commands and
a native `y` decision. A saved old-A/old-B session is followed by cold new-A and
new-B tabs before recovery. Recovery overwrites new-B with old-B, creates another
new-A row from a current-run checkpoint, and leaves the original new-A model at
`about:blank` while its renderer still shows new-A. The old checkpoint contains
only old-A/old-B. Evidence, the binary hash, owned request log and before/after
IDs are in `target/recovery-flow-probe/run/`. This experiment is separate from
the final dialog qualification and makes no fix claim.

A second probe records another consequence: immediately after recovery, a public
`session-save lazy-proof` writes `about:blank` for every unvisited restored row,
although the QML model still contains old-A/old-B URLs. Neither old URL survives
in the named snapshot (`target/recovery-flow-probe/run-lazy/`). This run also
selects a current-run checkpoint first and reloads new-B instead of choosing the
old session; source ordering changes the visible outcome. Both recovery probes
clean up their owned browser process groups and disposable profile roots.
Preserving those
tabs, identifying an untouched startup placeholder, and capturing previous-run
sources require their own complete implementation pass. Slow real filesystems
and workspace/geometry restoration are not qualified here.
The broader browser usability goal remains active.
