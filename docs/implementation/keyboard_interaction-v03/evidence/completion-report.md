# Keyboard interaction completion work

This report supplements the original slice records. Historical measurements and
failed attempts remain in the ledger. The final test logs and source revision
are recorded in `completion-validation.json`; qualification statuses remain
conservative when a required environment or participant is absent.

## Implemented repairs

- Generated resource inputs retain their timestamps when their bytes are
  unchanged, preventing the manifest from invalidating every subsequent Qt
  build.
- Runtime configuration prepares and validates the full candidate, projections,
  bindings and selector fingerprint before persistence or publication. A stale
  candidate or failed write before atomic replacement preserves the prior
  configuration, revision, policy and pending interaction. After replacement,
  failed directory sync reports committed/durability-uncertain and publishes
  the visible configuration. Ordinary successful writes report durable.
- Supported selector-family overrides can be inserted into an initially empty
  map. Startup validation checks the authored configuration, not safe defaults.
- Explicit local Hint selections can open the action palette. Automatic
  `--first` completion cannot mint local palette authority. Menu dismissal waits
  for item activation and checks the captured request identity.
- Scoped Hint and Grid handoffs preserve the destination lease through source
  mode teardown. Nullable receipt fields survive the WebEngine QVariant bridge
  through a bounded opaque JSON transport. Scoped Grid binds its exact owner
  during handoff and rechecks fresh geometry before native dispatch.
- Dense Hint overlays allocate an outline only for the active target,
  eliminating invisible per-target objects without changing marker layout.
- Missing Hint collection state and target projection were added. Prompt UUIDs
  use the Rust UUID implementation rather than an unavailable Qt function.
  Focused page-dialog controls forward Escape/Return to accept/cancel handlers.
- The CLI checks an entire expanded local-only interaction chain before sending
  any IPC frame, preventing partial navigation before rejection.
- Renderer termination clears active Hint authority and scoped/pointer state
  before recovery. Releasing held confirmation cannot replay on the reloaded
  page. Background renderer failures do not cancel another tab's Grid.
- Native click dispatch acquires page focus and rechecks the owner/surface before
  sending input. Hover retains its existing focus behavior.
- Supported permissions use the scoped Qt permission enumeration. Clipboard
  permission allow/deny remains governed by the configured request policy;
  unrestricted JavaScript clipboard flags are not enabled.
- S21 now waits on S20 as blocked_environment. A concrete S20 no-go makes S21
  not_applicable; missing input facilities do not imply a technical no-go.

## Application evidence

The new `tests/application` harness links the release Rust BrowserUi and loads
shipped QML resources. A disposable profile and loopback fixture exercise real
routing, policy, callbacks, native adapters and menus. There are no replacement
policy callbacks. Coverage includes Mixed shortcut delivery and recovery,
Pointer Hint entry/confirmation/cancellation, same-origin frame and open shadow
controls, popup routing, Target Search confirmation, modal restoration,
selector startup/reload races, persistence failure, scope handoffs, zoom/DPR
mapping and 1,000 Hint start/cancel cycles with forced GC and QObject counts.

The standalone IPC probe uses the release browser and public CLI to verify
restricted-chain rejection preserves the active URL and binding-explain reflects
actual Mixed ownership. Component-only native-insert results are not treated as
BrowserUi T010 evidence; the old overclaims in S21/S27 have been corrected.

## Research decisions

Fresh pinned editor, cross-origin-frame and accessibility probes are retained as
`completion-*-probe.log`. Editor APIs provide useful individual contracts but
Tiptap focus did not reach its sink and no generic adapter met the complete
production gate. Public frame evaluation provides exact handles but no stable
association between duplicate child metadata and embedding geometry. Qt native
accessibility targets are actionable, while WebEngine exposes no usable target
set in the probe. Existing bounded no-go decisions remain; these observations
do not rule out later platform changes or individually qualified adapters.

The label rerun retains the default: numeric labels save one gesture at 1,000
targets but relabel every retained target at the 1,001-width boundary. Modeled
gesture time is not human completion time or error evidence.

Two controlled baseline/current pairs use 10 warmups and 100 samples per
workload, with reversed second-pair order. No common 500-target callback row
crosses the combined 10 percent/10 ms investigation threshold. Raw samples,
viewport, user agent, flags, load and thermal metadata are preserved. Collector,
fixture and benchmark sources match the pinned S17 baseline; the unoptimized
overlay also matched it byte for byte.

The baseline and unoptimized current overlay both exceeded the 200 ms reference
ambition. Profiling exposed 500 outline delegates despite a single active
target. The production change retains only the active outline, preserving its
geometry, selection feedback and accessibility name. A dedicated 500-target
regression checks selection change, unknown selection and teardown.

| Zoom | Fresh baseline overlay p95 | Optimized overlay p95 |
| --- | ---: | ---: |
| 80% | 226.76 ms | 187.21 ms |
| 100% | 226.04 ms | 183.43 ms |
| 125% | 217.05 ms | 176.18 ms |

The final overlay samples use the same workload, viewport and Qt settings and
meet the reference ambition at every zoom. Collector, freshness, traversal and
payload budgets are unchanged. The Rust pipeline sampler also passed; its
numbers are debug-build measurements, not an optimized-build claim.

## Gates that still require external evidence

Physical/compositor-delivered Wayland input is distinct from Qt-generated
Wayland input and startup smoke. Platform IME/chooser interaction, authenticated
live sites, screen-reader participation and human label ergonomics were not
provided for this local-automation run. S19 double click remains disabled and
S20 has no capability decision until its required input evidence exists. S21
therefore remains blocked. Slice completion and strict pack completion must
not be inferred from local suite success.

The 20-trial interval experiment replaces a target during trusted pointerover,
after preflight but before the native press. Every trial delivered exactly one
trusted click to the replacement; there was no retry or fallback. Pointer
preflight therefore provides a bounded prior observation and current browser
owner, not immutable DOM identity across dispatch. This is the contract's
admitted observation-to-dispatch uncertainty, measured rather than hidden.

Active-renderer loss during held confirmation is covered through actual
termination and reload. Dispatch-in-flight loss, platform permission differences
and live-site behavior need their own matrix evidence. Fullscreen allowance was
observed with the public Qt feature enabled in the fixture; the browser default
was not changed. A trusted fixture event alone does not qualify APIs or native
choosers on another platform.

## Final local validation

Final source revision: `bc9284eb8a55f948f45846fa3f18635f8ae9c507`. Repository validation passed 886 Rust
and 160 QML cases, strict Clippy, formatting, architecture and generated-document
checks. Application validation passed 48 functional cases, separate DPR 2 cases
and 1,000 lifetime cycles; component native validation passed 59 cases. The real
release-browser IPC probe passed. Earlier compiler-contention and fixture setup
failures remain alongside successful reruns. The [validation ledger](completion-validation.json)
identifies exact commands and logs.
