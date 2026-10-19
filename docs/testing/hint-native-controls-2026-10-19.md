# Hint disclosure and native-control repair — 2026-10-19

## Behaviour

This repair starts from local commit `ec92817`. Current Reddit uses a focusable
`details` container and a native `summary` for collapsed comments. Previously,
both received hints at the same position; selecting the container focused it and
entered Insert without expanding the thread.

The collector now prefers an admitted native summary over its redundant
focus-only details container. The relationship comes from the first direct
summary child, recorded during the bounded physical traversal. A linear pass
filters redundant candidates before existing retained-record cleanup. It does
not merge coincident but distinct controls or transfer their labels. Missing,
hidden, covered, selector-excluded, and budget-starved summaries keep any
otherwise eligible focus target. Region/surface facets retain their contracts.

Native button/submit/reset/image inputs use the existing Button kind; checkbox
and radio inputs use the existing Aria kind. Both resolve to semantic activation.
The buttons family includes these inputs and native summaries. Text fields and
ordinary focus regions retain exact focus. The browser uses one DOM click,
leaving constraint validation, radio grouping, cancelled defaults, form submission
and reset to Chromium; this does not claim trusted physical-pointer delivery.

Input type is captured in the internal retained record, including native label
and editor proxy receivers, and checked again for freshness and pointer preflight.
Captions use button/submit/reset values or image-input alt text only after the
existing accessible-name sources. Editable, checkbox/radio and editor values
remain excluded. Existing Unicode/text bounds and generic fallbacks apply.

The private page-interaction resource contract advances from 4 to 5 and the
BrowserScripts resource from 22 to 23. No IPC, public payload or persisted-data
schema changes are introduced.

## Regression evidence

Artifacts are local under `target/hint-controls-fix/` and excluded from Git.
The new deterministic fixtures run in offscreen Qt WebEngine / ApplicationWorld.

- The final new suite against captured original scripts: **12 passed, 19 failed**
  (`controls-baseline-final.log`). Failures expose duplicate focus targets,
  missing native button discovery/captions, incorrect control kinds, retained
  redundant records and missing input-type freshness checks.
- The same new suite against corrected scripts: **31 passed, zero failed**
  (`controls-complete.log`). Cases cover document/shadow/frame disclosures,
  open/closed repetition, nested disclosures, selector/budget/visibility fallbacks,
  scoped collection and refresh retirement, all/inputs/buttons discovery,
  native effects, cancellation, disabled/inert/replaced receivers, caption privacy
  and bounds, label proxies, type changes and document navigation.
- Existing validated label-proxy coverage now expects an actionable Aria kind for
  an ordinary visible checkbox, preserving one control hint and one native action.

## Qualification

- Release build passed (`release-build.log`, 12m 18s). Embedded-resource parity
  passed against that engine in the production integration run.
- The isolated live Reddit replay passed (`live-reddit-ready.log`, three QTest
  passes): exactly one summary receiver remained and one semantic click opened
  the collapsed comment. The first attempt ran before Reddit had rendered its
  comments; the replay waits for the actual disclosure DOM before probing.
- The first repository gate returned 661 QML passes and one callback timeout in
  the existing 55,000-node/32-selector stress fixture while compilation was busy.
  An unchanged isolated repeat passed in 3.7s, with 2.5s collection, 5,000 output
  candidates and the 50,000-inspection bound (`selector-stress-repeat.log`).
- The new production regression failed against the original engine
  (`native-before.log`). In the initial updated-engine run, all primary-window
  activation rows and resource parity passed. The new secondary fixture incorrectly
  used `nativeUrl` for an HTTP page; it now waits for the owned renderer URL.
  DPR 2 also exposed a fixture dependency on asynchronous editor blur when testing
  a subsequent focus-only region. Focus-only and editable checks now run in that
  order so each tests its intended contract independently. Production focus-mode
  lifecycle code is unchanged. Initial failures remain in `native-qualification.log`.

- Final production keyboard gates passed (`native-qualification-final.log`):
  11 passes at normal scale and six at DPR 2; both CTest gates passed in 58.79s.
  This includes primary windows at 80/100/125% zoom and a secondary window,
  exact event counts, expected modes and action previews, resource parity,
  keyboard activation, passive refresh and held-confirmation coverage.
- The final repository gate passed (`xtask-check-final.log`, 416.81s):
  **1,116 Rust tests passed, two ignored; 662 QML checks passed, zero failures**.
  This includes QML lint, architecture checks, generated documentation, native
  profile/download-source checks, release-input/license checks and formatting.
  A preceding run (`xtask-check-version-assertion.log`) found the stale Rust
  resource-version assertions after its QML suite passed. Those assertions now
  expect the intended 23/5 versions and verify the matching facade guard.
- Both focused native gates are included in the existing CI browser-surface
  selection; CTest discovery verifies that selection (`ci-native-selection.json`).

## Benchmark comparison

Captured baseline scripts match `ec92817` byte-for-byte. The existing
`HintPerformance::test_T043_samples:1000-100` scenario ran sequentially with
original and corrected scripts: 1,000 buttons, 10 warmups and 100 measured
samples each, Qt 6.11.2, offscreen software rendering on the same host.

| Median milliseconds | Baseline | Corrected |
| --- | ---: | ---: |
| Instrumented collector | 108.9 | 114.7 |
| Collection round trip | 149.291 | 149.110 |

The collector median increased about 5.3%; round-trip medians were effectively
unchanged in this run. These are instrumented dispatch/collection observations,
not overall browser-speed measurements, and background host activity was not
controlled. The new bookkeeping remains bounded and linear; the full selector
stress test also passed. Both benchmark runs passed their candidate-count,
freshness, pointer-preflight and exactly-once click assertions.

Logs are `benchmark-baseline-final.log`, `benchmark-after-final.log`, and
`benchmark-summary.json`. The original exploratory benchmark overlapped other
work and was interrupted; it is excluded. The first captured-baseline invocation
failed before sampling because its scratch QML directory import lacked a file
URL; the corrected invocation is the comparison above. All final validation
steps returned zero (`final-validation.json`).

## Installation

The validated release is installed at `~/.local/bin/ferric-browser`, with updated
user guides and changelog. Its SHA-256 matches the workspace release executable:

`deca1ca925c97bd0275ddc4d1308c587ecc997a08b05fc0b7ad9e304632b97c5`

The previous executable, desktop/icon assets and installed documentation were
copied and checksum-verified before replacement under:

`/home/tom/Backups/ferric-browser/20261019-121302`

That directory contains `manifest.json`, `README.txt` and a checksum-verifying
`restore.py`. Quit Ferric before running that restorer to roll back. The earlier
October 19 10:45 backup is retained independently.

Desktop/icon cache refresh, installed version/help, isolated temporary-profile
JSON diagnostics and linked-library resolution all passed (`installation.json`
and smoke logs in the backup directory). Browser profiles and settings were
not changed. Quit all running Ferric instances and reopen the application to
load the new executable. Source changes remain uncommitted.
