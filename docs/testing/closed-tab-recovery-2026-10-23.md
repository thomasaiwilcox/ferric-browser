# Closed-tab recovery and presentation

Status: scoped recovery, offscreen, owned Wayland, resource, unit, QML and lint qualification passed, 2026-10-23.

## Reproduction

The production base is the completed Stop-state revision. The corrected native
probe creates three tabs, closes the middle one, and presses `u`. It reopens at
index two instead of index one (`target/tab-recovery-baseline-smoke-3.log`, two
setup/cleanup passes, one failure, 3.88 s).

The full baseline covers position, pinned state, mute and zoom independently,
using `u` and Ctrl+Shift+T in primary, secondary, private, temporary and popup
hosts. All 40 behavior rows fail; the two setup/cleanup checks pass in 176.81 s
(`target/tab-recovery-baseline-native.log`). The retained descriptor and actual
renderer both lose mute/zoom; pinned tabs reopen unpinned. Pinned cases use an
explicit ID-based close because the ordinary close command protects pinned tabs.
All reopening uses the configured keys.

A focused QML check captures settings when a view component is created. It sees
default pin/mute/zoom even when the descriptor and eventual metadata refresh
contain saved settings: seven passes and one failure in 18 ms
(`target/tab-recovery-baseline-qml.log`).

Two initial native smoke attempts stop in fixture setup because they use
CLI-style `open --target tab` through the lower-level GUI command entry point.
The fixture uses the canonical `tab-open` command and drains host actions
before observing tabs. These setup failures are not evidence about Undo. The
temporary case explicitly enables ephemeral-profile mode. The full baseline
runs against the unchanged, hashed release archive and native executable in
`target/tab-recovery-baseline-artifact-hashes.json`; Rust source edits made
while that matrix ran do not alter either binary. QML production sources remain
unchanged until that baseline finishes.

## Compiled recovery findings

The first corrected release build passes all 900 selected release units, 1,198
QML checks and strict Clippy. All 40 compiled offscreen keyboard recovery rows
pass. Its two checkpoint cases fail with four Core tabs instead of two because
the probe invokes startup recovery while three tabs remain live. The fixture
now retires the extra test tabs before recovery, leaving the single initial tab
used at startup. The last live closed record differs from the saved record, so
the recovery check still requires a disk read.

One subsequent attempt aborts when the test helper calls JavaScript before the
replacement view exists. The QtTest stack and `coredumpctl info 1666021` identify
`ApplicationTests::evaluate` calling `QMessageLogger::fatal` after a null-view
TypeError. There is 10 GiB available memory and no kernel OOM entry. No core is
extracted. Readiness waits precede page JavaScript; see
`target/tab-recovery-checkpoint-crash-facts.json` and its linked logs.

With that setup corrected, the probe exposes a real recovery error: both current
and version-2 checkpoints assign the pinned neighbor and selected tab each
other's saved addresses. Two behavior rows fail in 5.09 s
(`target/tab-recovery-checkpoint-startup-native-2.log`). Two focused QML checks
also fail: pinning reorders a replacement plan, and appending pinned tabs loses
row association and lazy-loading state. That run has eight passes and two
failures in 13 ms (`target/tab-recovery-restore-order-baseline-qml.log`).

The restore payload now includes each newly allocated Core tab ID. QML associates
rows and selected-page loading by that identity before reconciling pin order.
Selected navigation uses the saved URL separately from the formatted metadata
row. The focused suite passes all ten checks in 12 ms. Compiled checkpoint
checks also select the lazy pinned neighbor and require its actual page address.
The previous build, source hashes, logs and artifact hashes are retained under
`target/tab-recovery-pre-id-`; they do not qualify the final correction.

A later full offscreen run passes 43 checks and fails one immediate
`flush_durable_state()` assertion before the legacy checkpoint read
(`target/tab-recovery-final-application_tab_recovery-before-save-wait.log`,
250.26 s). That helper can reject admission while an automatic checkpoint is
pending, or return false when another change has dirtied the captured generation.
The first attempt records no finer failure reason. The fixture now waits up to
five seconds for successful save completion and logs a non-ready status; every
on-disk metadata and recovery assertion remains required. Production sources,
release units, full QML and lint results are unchanged by that fixture correction.

This earlier matrix qualifies the single-tab startup state. Deferred startup
recovery with existing live work was subsequently corrected and qualified in
[the startup recovery pass](startup-recovery-2026-10-23.md). Named-session
replacement retains its separate unsaved-page policy.

## Change

Closed-tab descriptors retain their position within the pinned or unpinned
group, pinned/muted state and zoom in hundredths. Reopening applies those
settings and a bounded group position through the existing reducer events
before publishing the new tab to QML. The common closed-result activation path
serves both Undo and Switcher reopening. Reopened pages use fresh document
identities and the existing safe-GET descriptor policy. A fresh tab is allocated
except when the last-tab follow-up below proves that the automatic fallback
can be reused.

The tab-view controller seeds new rows from the Core descriptor before creating
a renderer, including mute and zoom. Existing views retain their identities
when tab ordering changes.

Restore rows retain their runtime tab IDs through pin regrouping, including
appended sessions. Loading only the selected restored page uses that ID.

Normal-profile checkpoints persist the same presentation fields. Session format
3 reads format 2 with safe defaults for absent closed-tab settings and upgrades
it in memory. The source file remains unchanged until an ordinary atomic save;
its old bytes remain the previous generation. Existing format-1 and unknown
future-format rejection remains. Closed-tab zoom and group positions are bounded
before writes. Private and temporary descriptors retain their existing memory-only
policy.

## Verification

Frozen manifests cover 65 production/embedded-test sources, 12 fixture sources,
and the CI workflow separately. Eight source files differ from the Stop-state
base: the existing tab-view controller, close capture, common closed activation,
closed-tab descriptor, checkpoint save/restore and storage snapshot/tests.

The initial focused QML suite passes eight checks in 19 ms after correction;
the final restore-order suite passes ten in 12 ms. All 107
storage units pass, including settings round-trip through the storage worker,
version-2 read and fallback, preservation of the previous generation, and invalid
settings leaving a valid snapshot untouched. Strict storage Clippy passes in
4.81 s.

The final native matrix checks each setting, the original neighbors, a fresh
page/document identity, actual native mute/zoom and page focus. Its additional
two checkpoint cases consume the live undo record, flush its removal, then read
the saved on-disk generation through the production recovery worker before Undo, with a single initial live tab matching startup.
One case uses format 3 and one removes the new fields from a format-2 file. These
exercise disk writes, worker reads and renderer replacement within one owned
process.

The final corrected all-target release build passes in 13 min 36 s. Its exact
unit executables pass storage 107, Core 169, runtime nine, engine 504 and CLI 111
checks (900 total), with one opt-in engine benchmark ignored. The full QML suite
passes 1,200 checks with forced GC in 222.77 s. A targeted compiled checkpoint
run passes both current and version-2 cases, including actual navigation to the
lazy pinned neighbor: four Qt checks in 17.23 s
(`target/tab-recovery-checkpoint-fixed-native.log`).

The actual CI browser-surface filter selects both new recovery gates exactly
once among 23 entries (`target/tab-recovery-ci-selection.json`); the hosted CI
workflow itself is not run here. Documentation generation passes.

All final compiled gates pass. Counts include Qt setup and cleanup:

| Gate | Qt passes | Qt runtime |
| --- | ---: | ---: |
| Recovery, offscreen | 44 | 123.42 s |
| Recovery, owned Wayland | 44 | 119.11 s |
| Close requests, offscreen | 16 | 24.04 s |
| Tab focus, owned Wayland | 26 | 53.03 s |
| Privacy identity, owned Wayland | 11 | 41.88 s |
| Full Switcher, offscreen | 108 | 274.24 s |
| Switcher live updates, owned Wayland | 7 | 18.48 s |
| Stop, owned Wayland | 30 | 74.93 s |
| Embedded-resource parity | 3 | 1.85 s |

The 289 canonical native Qt checks have no failures or skips. The
separate four-pass targeted checkpoint run is not added to that total. Bytes
and manifest hashes match all 115 embedded source files. Strict Clippy passes
with `-D warnings` in 6 min 29.45 s. Formatting, documentation generation and
whitespace checks pass. The final audit checks the frozen production, fixture
and CI hashes, exact release artifacts and actual gate totals; see
`target/tab-recovery-final-audit.log`, `target/tab-recovery-final-facts.json` and
`target/tab-recovery-artifact-hashes.json`.


## Last-tab Undo follow-up

Status: last-tab Undo qualified against the canonical release archive and CLI, 2026-10-23.

The current qualified release leaves two tabs after closing the sole HTTP page
and pressing either `u` or Ctrl+Shift+T: the restored page and the automatic
start tab. The new native baseline has two setup/cleanup passes and two failures
in 35.27 seconds (`target/last-tab-undo-baseline-native.log`). Archive and CLI
hashes match the startup-recovery release. A separate focus check exposes the
same-view native-to-web transition: 20 passes and one failure in 5.50 seconds
(`target/last-tab-undo-focus-baseline.log`).

The close acknowledgement captures the reducer-created fallback target. Shared
closed-result reopening reuses it only while its owner, selected sole-tab
membership, live target, default settings and single initial native-history
entry still match. The native view reports whether its start page has no active
input or unfinished favourite work. Unknown UI state and queued work prevent
reuse. A changed document cannot qualify merely by returning to the same URL.
The captured marker is consumed on reuse and cleared on profile replacement.

Reopening applies saved settings before navigation. Reused views receive a
captured zoom request before the safe GET, and the automatic start-page history
entry is discarded. The shared focus controller follows native-surface changes
within the same view, using its existing modal and browser-input guards.

The first release passes all 1,032 selected Rust checks, strict Clippy and the
public CLI close/undo probe. Its native smoke passes eight checks and fails two:
the restored document does not have focus even though the WebEngine ancestor
does. A local diagnostic identifies a hidden guide row as the active item.
Qt's [Item focus contract](https://doc.qt.io/qt-6/qml-qtquick-item.html#activeFocus-prop)
allows a focus-scope ancestor to report active focus while a child receives
input. The native assertion therefore checks `document.hasFocus()` as well.
Logs, inputs and artifacts from that revision remain under
`target/last-tab-undo-pre-focus-`.

The correction disables hidden start-page controls and hands page focus to an
eligible descendant from the public focus chain. The descendant must remain
inside the requested view. Existing modal and browser-input guards run before
the handoff, including overlay and mode returns. This requires no private Qt
type lookup or JavaScript focus injection. The focused suite passes 23 checks,
including descendant handoff, overlay return and input/modal protection. The
local-QML native diagnostic passes all 41 checks in 108.74 seconds with no
test-side focus repair. Those diagnostic passes do not qualify embedded QML.
All 1,207 source QML checks pass in 219.36 seconds for this revision.
Strict Qt/CLI Clippy passed before rebuilding. The final strict Clippy command
and canonical release gates also passed; audited totals follow.

The committed native matrix contains 39 scenarios. Ten untouched cases cover
both shortcuts in primary, secondary, private, temporary and popup hosts. The
other cases preserve incoming page drafts and QObject identities, navigation
back to start, explicit native tabs, modified tab settings, unfinished favourite
edits and a later close/undo sequence. Pin/mute/zoom, actual renderer settings,
web-document focus and history are checked. An early eight-scenario smoke run
is separate from the full offscreen and owned Wayland totals.

Frozen manifests and final results are under `target/last-tab-undo-`. The CI
filter selects both new gates exactly once among 30 entries; hosted CI is not
run here. The broad usability goal remains active.


| Final check | Result |
| --- | --- |
| Release Rust unit checks | 1,032 passed; one optional Qt benchmark ignored |
| Full QML suite | 1,207 passed (219.36 s) |
| Native Qt checks | 360 passed, including 41 offscreen and 41 owned Wayland last-tab checks |
| Public CLI close/undo | Passed; restored one page using the captured fallback ID (6.76 s) |
| Startup restart scenarios | 2 passed; two forced exits and recoveries per scenario |
| Strict Clippy | Six packages, all targets, warnings denied (390.39 s) |
| Formatting and whitespace | cargo fmt --all -- --check and git diff --check passed |

| Native gate | Passed | Seconds |
| --- | ---: | ---: |
| `application_last_tab_undo` | 41 | 216.32 |
| `application_last_tab_undo_wayland` | 41 | 128.94 |
| `application_startup_recovery` | 6 | 16.80 |
| `application_startup_recovery_wayland` | 6 | 16.76 |
| `application_session_recovery` | 10 | 24.38 |
| `application_session_recovery_wayland` | 10 | 24.10 |
| `application_tab_recovery` | 44 | 128.07 |
| `application_tab_recovery_wayland` | 44 | 122.41 |
| `application_close_requests` | 16 | 24.29 |
| `application_switcher` | 108 | 273.94 |
| `application_tab_focus_wayland` | 26 | 54.38 |
| `session-consumers` | 5 | 9.29 |
| `resource-parity` | 3 | 1.75 |

The early native smoke passes 10 checks and is excluded from the 360-check
total. The public CLI probe and two startup process scenarios are counted
separately. Resource parity confirms embedded QML matches the checkout;
native binaries link `target/release/libferric_browser_engine_qt.a`.
Frozen source, fixture and CI manifests cover 75, 19 and 1 unchanged files.
Final facts and exact artifact hashes are in
`target/last-tab-undo-final-facts.json` and
`target/last-tab-undo-artifact-hashes.json`.
