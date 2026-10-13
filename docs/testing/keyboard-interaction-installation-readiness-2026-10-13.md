# Keyboard interaction installation readiness review

**Current status: the scene mapping defect is repaired and the browser is installed
for manual testing.** Product source commit
`c9e734a60e05c46314380b2619f5673a720d6de0` captures the view's mapped corners,
chosen point, window geometry, global point and device pixel ratio. Each delivery
guard compares that mapping before a press. Changed mappings reject an initial
gesture; interrupted gestures retain bounded cleanup and report uncertainty.

The permanent `pointerAdapterSceneMapping` regressions cover eight changes at
three stages, plus a stable transformed view. All 24 negative rows failed before
the repair and pass afterward. The full native suite passed 94 cases. A fresh
release build and relinked application suite passed 56 functional cases, the
separate DPR 2 cases and 1,000 lifetime cycles. QObject counts returned from 3368
to 3368 and retained maps, observers, windows, scope and tracking were cleared.
Formatting, source whitespace, packaging and pack integrity checks passed.

The validated executable was installed atomically to
`/home/tom/.local/bin/ferric-browser`, with the desktop entry, icon and ten
documentation files updated under `/home/tom/.local/share`. Installed file hashes
match the source inputs. HTTP/HTTPS default handlers remain unchanged. The
previous installation is retained under
`target/scene-mapping-fix/previous-installation/` for rollback.

Installed help, disposable diagnostics JSON, desktop-file validation and real
CLI/IPC checks passed. The installed executable also passed startup and CLI/IPC
checks in a private headless Weston Wayland session. That session supplies startup
evidence, not physical keyboard/pointer evidence. Raw logs and the
[validation ledger](../../target/scene-mapping-fix/validation.json) are under
`target/scene-mapping-fix/`. Rust/QML workspace gates recorded in the earlier
validation remain historical; this C++ correction was validated by the fresh
native and packaged application suites above.

The build is ready for disposable-profile manual testing with
`ferric-browser --temp-basedir`. Physical input, IME/choosers, live sites and
accessibility still require qualification. Double-click and drag retain their
existing gates, and the documented DOM/native targeting interval remains.
Nothing was pushed or published.

## Original review before the repair

The remaining sections preserve the review of `5afee04` and its failing probes.

**Original verdict: fix one remaining native input isolation defect before approving an
installation for Pointer Hint and Grid testing.** The release executable launches
outside the repository, packaging checks pass, and the existing automated suites
pass. An additional adversarial probe nevertheless delivers a button gesture to
the wrong Qt receiver after the target's scene position changes.

This is a self-review of local HEAD
`5afee0405ca85046f47036eb4afe792d6296b71e` against freshly fetched `origin/main`
`e9a576fd375927733b200f91a6387dea4698840d`: 73 local commits, no remote commits
missing locally. The tracked crate diff covers 152 files. HEAD's product source
is unchanged from `5410c038409a835d271a08b42e96ea6dd434d6f9`; the final commit
records evidence. Unrelated untracked `aqtinstall.log` and `tools/` are excluded.

## P1 Native dispatch retains coordinates after observed scene movement

**Confidence: reproduced using the production C++ adapter.**

[browser_page_pointer_adapter.cpp](../../crates/ferric-browser-engine-qt/src/browser_page_pointer_adapter.cpp)
captures `windowPosition` once at lines 228–230, sends the move, then uses that
captured position for the press. `currentSurface()` at lines 257–260 checks
receiver identity, eligibility, revisions and request identity, but does not
check that the target-to-window mapping still matches. The target connections at
lines 71–79 observe width, height and visibility; target translation and ancestor
translation do not advance the surface revision.

The probe creates a real QQuickWindow with two mouse-accepting items. The adapter
targets a 100 by 100 item at scene origin and dispatches a left gesture at local
point `(50, 50)`. A window event filter moves the target 200 pixels horizontally
during the dispatched move. A second case moves its parent instead. Neither case
resizes, replaces or hides the target. In both cases, the subsequent press and
release reach the background item at the old window point:

```text
target-translation:   scene_x=200 revision_changed=0 target_presses=0 background_presses=1 background_releases=1 ack=delivered
ancestor-translation: scene_x=200 revision_changed=0 target_presses=0 background_presses=1 background_releases=1 ack=delivered
```

Both safety assertions fail with exit code 1. The stable control passes with one
target press, zero background presses and a delivered acknowledgement. The
[probe source](../../target/readiness-review/geometry-probe/main.cpp),
[build definition](../../target/readiness-review/geometry-probe/CMakeLists.txt)
and [results](../../target/readiness-review/geometry-probe.json) are ignored review
artifacts. The probe links the actual adapter source, without substitute dispatch
logic.

The shared adapter serves both Pointer Hint and Grid. A layout change can therefore
aim a native button gesture outside the intended page view, including at browser
chrome occupying the old point. This violates C06's observed geometry/isolation
requirement. It is separate from the admitted asynchronous DOM targeting interval:
the Qt scene mapping itself has changed synchronously and is available to the
adapter. The missing position tracking also exists on the remote baseline; this
is an inherited gap in the shared adapter, not a new selector/reload regression.

The correction should validate the captured effective scene/window mapping
immediately before a press, covering ancestor movement as well as direct target
movement. A changed mapping must reject the gesture before any button event.
Preserve bounded cleanup after an already attempted press, one terminal outcome
and no retry or alternate transport. Add permanent regressions for both reproduced
cases, then cover transforms and window mapping changes alongside stable delivery
and existing invalidation cases. Connecting only the target's x/y signals would
leave the reproduced ancestor case open.

## Previously reported repairs

The three findings in the
[earlier review](keyboard-interaction-review-2026-10-13.md) have valid repairs and
permanent regression coverage. Move-time blocking, disabling, target replacement,
resizing and deactivation prevent the first press. Selector discovery consumes the
canonical nested Rust policy, including inheritance, replacement and empty
replacement. Reload activates reload settings through the configuration
transaction while preserving startup/restart deferral and rejection of invalid,
stale or timed-out selector candidates.

The new scene-movement case extends the native geometry coverage beyond those ten
invalidation rows. No additional reproduced configuration, selector, resource or
CLI/IPC integration defect was found in this review.

## Validation

Fresh checks in this review used the unchanged product source and existing release
build. UI suites ran without concurrent compilation.

| Check | Result |
| --- | --- |
| Native component suite | 69 passed, zero failed |
| Production application functional suite | 56 passed, zero failed; lifetime slot intentionally run separately |
| DPR 2 application suite | 5 passed, zero failed, including three zoom cases |
| Application lifetime suite | 1,000 Hint cycles; QObject count 3368 to 3368; retained elements, observers, windows, scope and tracking cleared |
| Relocated release executable | Runtime libraries resolve; disposable diagnostics JSON parses; GUI startup and real CLI/IPC probe pass with working directory outside the repository and QML import overrides removed |
| Release build resource manifest | All 82 resource hashes match current source |
| Arch metadata and release artifacts | Passed |
| Pack integrity and evidence records | Passed |
| Formatting and crate diff whitespace | Passed |
| Additional scene movement probe | Stable control passes; target and ancestor movement safety cases fail |

Fresh raw logs and diagnostics are in `target/readiness-review/`. The recorded
[full workspace validation](../implementation/keyboard_interaction-v03/evidence/review-repairs-validation.json)
on the same product source passed 889 Rust and 161 QML cases, strict workspace
Clippy and repository gates. Those recorded workspace checks were not repeated
during this review. Existing QML context/cleanup warnings and Chromium teardown
messages remain in the raw application logs; these runs did not establish a
user-visible teardown failure. Automated suite success does not cover the newly
reproduced movement case.

## Installation and qualification

The implementation follows the intended shared registry, input router, native
adapter, configuration authority and ranking architecture. The selector/reload
repairs now support the declarative policy vision. The remaining Qt mapping gap
prevents approval of the native input isolation behavior.

After correcting that gap and passing its regressions and integration suites,
proceed with a disposable-profile installation using `--temp-basedir`. Physical
Wayland delivery, real IME/AltGr/dead-key input, platform choosers, prompt recovery,
authenticated live-site journeys and accessibility remain manual qualification
work. They are appropriate next tests, rather than evidence supplied by offscreen
Qt-generated input. Clean package installation and desktop launch also remain to
be qualified for release.

Double-click remains disabled and drag remains blocked. The documented DOM/native
observation interval remains: the application experiment again recorded replacement
receivers receiving one trusted click in its 20 trials. Fixing the Qt mapping gap
does not make DOM/native targeting atomic. The pack's strict completion audit still
has 22 qualification-status errors; neither installation testing nor this review
promotes those statuses to complete.

Product source was not modified. Nothing was installed, pushed or published.
