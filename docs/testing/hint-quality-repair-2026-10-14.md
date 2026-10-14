# Hint quality repair — 2026-10-14

The repair implements the [hint quality plan](../implementation/hint-quality-plan-2026-10-13.md)
against `ff9c69b`. The prior installed browser was built from `c9e734a`.
The [original investigation](hint-label-investigation-2026-10-13.md) remains a
historical record; its Reddit snapshot is not the same page state as the new run.

## Changes and user behavior

- Ordinary discovery paths enforce the captured selector family, including assigned
  roots. Plain slotted containers cannot become labels. Completed physical scans
  and per-scope link summaries are reused within a collection, preserving shadow,
  frame, scope and depth boundaries. Work remains bounded by existing limits.
- One interaction reserves labels at its initial width. Page updates preserve
  prefix, text mode/query, selection when still valid, and the meanings of surviving
  labels. Removed labels remain reserved; repurposed identities receive a new label.
  At capacity, existing labels remain usable and a notice requests restarting hints
  for new controls. Reservations and placement caches are bounded to 5,000 labels.
- Passive refresh cannot activate a candidate, including under `auto_follow=always`.
  Rapid selection consumes its binding immediately; a repeated selection before
  recollection is rejected. Every refreshed snapshot still creates fresh session
  permits; label continuity does not extend execution authority.
  An accepted Enter press cancels publication of an older in-flight passive
  collection for that gesture. Its late callback cannot replace the held pointer
  session. Release still validates the current DOM facts and native surface.
  The deterministic regression failed before this correction (`selecting` replaced
  `awaiting-confirmation-release`) and checks a single trusted click after repair.
- Names use bounded `aria-labelledby`, `aria-label`, associated labels, alternate
  text, visible text and title lookup. Editable contents and field values are
  excluded. Unicode names fit the existing 512-character/2,048-byte boundary.
  Focus-only widgets, including structural-role regions, use exact focus.
- Presentation facts are optional and validated independently of activation.
  Wrapped controls retain up to eight visible fragments and an anchor in one
  fragment; ordinary single-box controls use their existing validated geometry.
  Same-origin frame projection handles supported positive axis-aligned scaling,
  clipping, borders and padding. Pointer preflight uses the same inverse content
  mapping for supported frames, retaining stricter native transform/zoom rejection.
  Rotated/perspective children do not receive misleading coordinates.
- Badge sizes use font metrics. Surviving placement is reserved before new targets
  are laid out; short connectors identify small controls. The keyboard selection
  has one target outline and a plain-text role/name/action preview. Hover cannot
  redirect keyboard selection and does not dispatch webpage hover/focus/click.
- Actual badge collisions form selectable stacks. All members retain their own
  labels and typed actions; Space cycles them, and the expanded list is virtualized.
  Only displayed representatives create badge objects. Page scrolling gets a named
  edge affordance. Collection-limit notices are concise and do not repeat on each
  refresh. Generated user guidance and the changelog describe the behavior.

`HintPresentation.js` is a presentation-only helper. Rust retains identity,
interaction, intent, permits and activation policy. Existing hint commands,
alphabet, selector overrides, families, scope workflows and native scene-mapping
safeguards remain in their established paths. There is no Reddit-specific rule or
alternative label strategy.

## Scope amendment

This authorized repair extends S25 with T058–T061 for defects in existing default
hints; the earlier no-go on alternative label strategies remains unchanged.
Besides the documented hint anchors, the work changes core exports/user guidance,
RuntimeBase/Chrome, the status bar, typed payload boundaries and production/QML
regression harnesses. The new `qml/scripts/HintPresentation.js` isolates pure
layout facts from execution authority. Generated help, test-matrix attachments,
pack hashes and release records follow these changes. No new dependency, remote
service, command, registry or label alphabet is introduced.

## Automated qualification

Evidence is local under `target/hint-quality-fix/`, outside version control.
Qt is 6.11.2 with Chromium 140. The Qt runs use a software-rendered offscreen
window and ApplicationWorld scripts; they are not physical Wayland input.

| Check | Result | Evidence |
| --- | --- | --- |
| QML/WebEngine functional suite | 182 passed, no failures | `qml-final.log` |
| Native key/pointer interaction suite | 94 passed, no failures | `native-final.log` |
| Geometry at DPR 2 | 9 passed, no failures | `geometry-dpr2.log` |
| Production application | 61 passed, no failures; opt-in lifetime case skipped here | `application-qualified.log` |
| Scoped native coordinates at DPR 2 | 5 passed, no failures | `application-dpr2-qualified.log` |
| 1,000-cycle lifetime | 3 passed; QObject count 3,384 → 3,384, retained tracking cleared | `lifetime-qualified.log` |
| Full repository gate | Passed; 898 Rust tests, two explicitly ignored opt-in cases, QML lint and generated docs checked | `xtask-qualified.log` |
| Strict Clippy for Rust workspace/all targets | Passed | `clippy-final.log` |
| Release build and embedded resource parity | Passed; all 83 resources match the source | `release-qualified.log`, `resource-parity-qualified.json` |
| Arch metadata, release inputs and license policy | Passed | `package-arch.log`, `package-artifacts.log` |
| Implementation pack structure and integrity | Passed | `pack-evidence-qualified.json` |

The new held-confirmation regression failed deterministically before the final
correction and passed with the rebuilt embedded QML. Earlier interrupted/failing
runs are retained as negative evidence, not counted as qualification. Existing
`clearPageDialogForView` teardown-context warnings also appear in the prior
`target/scene-mapping-fix/application-full.log`; they are not claimed as repaired
by this work. The successful final suite still verifies state and effect counts.

New regressions cover selector admission, shadow discovery under budget pressure,
Unicode/privacy, wrapped fragments, exact focus, scaled/padded/nested frames,
unsupported rotation, stable reservations at width/capacity boundaries, unchanged
prefix/query, absence of refresh activation, consumed replay, stationary placement,
actual collision reachability, passive hover and expanded-stack refresh.
T058–T061 are linked to S25 in the canonical matrix. Historic registry statuses
are not rewritten to imply new human or release qualification.

## Performance

Common-case measurements use ten warm-ups and 100 measured samples, nearest-rank
p95, on a quiet host. Collector and overlay stages are separate measurements;
the numbers below are not a single combined time-to-interaction result.

| 500-target workload | 80% zoom | 100% zoom | 125% zoom |
| --- | ---: | ---: | ---: |
| Collector browser round-trip p95 | 101.50 ms | 101.61 ms | 94.60 ms |
| Overlay time-to-visible p95 | 117.88 ms | 107.97 ms | 63.40 ms |
| Overlay layout/model update p95 | 106.21 ms | 94.47 ms | 50.13 ms |

Both stages meet the required 200 ms gate. See `collector-optimized.log`,
`overlay-final.log` and `performance-summary.json`.

A same-host 100% zoom comparison with the previous `ff9c69b` script bundle measured
86.29 ms collector p95: the final repair adds 15.32 ms (17.76%). This crosses the
investigation threshold. Phase/payload measurements identified duplicate projection
and geometry payload, hidden stack badge delegates and a duplicate show-time layout
pass. Removing them reduced the initial repaired collector from 141.95 to 101.61 ms
and the overlay from over 500 ms to 107.97 ms. Bounded naming/Unicode, uniform
admission and client-fragment inspection still have a cost; no claim of zero
collector regression is made. The budget retains substantial headroom.

The 1,000/5,000-target and mutation runs are reported separately. They run with
compilation/other qualification activity and are stress observations, not quiet-host
hardware acceptance. The completed initial stress run preserved all requested
candidates and exactly one explicit semantic click per measured action. A final
run covers the single-box payload optimization (`scale-final.log`): p95 round-trip
894.47 ms at 1,000 targets, 5,829.67 ms at 5,000 targets and 892.49 ms for the
1,000-target mutation workload. All candidate-count and exactly-once action checks
passed. These values reflect the deliberately loaded host. Dense 5,000-target discovery remains a heavy workload rather than
meeting the 500-target latency guarantee.

## Anonymous Reddit and installation

The private-profile probe exports only kind/geometry and counters. It saves no
feed text, names, destinations, field values or account data. A fresh anonymous
homepage produced 29 candidates, zero selector-policy misses, zero coincident
rectangle pairs and no truncation (48,812 inspected work steps). Live content
varies; this is supporting qualification, not a controlled before/after usability
study. A second fresh page state produced 41 candidates with zero policy misses,
zero coincident pairs and 8,200 work steps. In one renderer task its target set
matched an extended-budget reference exactly: zero lost or extra targets.
See `reddit-final.log` and `reddit-equivalence.log`; changes between those live
states are not attributed to the repair.

The release artifact passed application qualification and is prepared for per-user
installation. Installed-binary evidence is recorded after atomic replacement. The per-user install uses atomic replacement, backs up prior files
and verifies the installed hashes. The running user session is not restarted.

## Remaining manual qualification

Physical Wayland keyboard/pointer input, screen-reader behavior and comparative
human task measurements remain unqualified. After restarting the installed build,
verify identifying/focusing Reddit search, opening a post and comments, distinguishing
vote/menu controls and selecting the intended scroll area. Use owned fixtures for
state-changing actions. Repeat on an ordinary document and dense application page
at the user's actual zoom/display scale. Record wrong-target attempts and task
completion/time before claiming measured usability gains.

Automated effect/identity checks, anonymous page counters, screenshots and nested
Wayland startup do not substitute for those measurements. This record supports
installation and user testing, not a claim that every website is bug-free.
