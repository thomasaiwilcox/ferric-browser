# Reddit initial-resize invalidation — 16 November 2026

The installed `f3c98058f2a14e79` build failed all three real Wayland `f`
attempts on the owner's already-loaded Reddit tab, including returning from
YouTube. The Reddit policy was active and valid; the renderer was healthy.

The ordinary stage captured 59 candidates and exhausted its 49,718-inspection
grant. Each new ResizeObserver then delivered its initial size notification,
even though those targets had not changed. Treating that notification as a
resize made the pending inventory dirty. The refresh had zero inspection
allowance and failed, including the one fresh-request retry. The earlier
successful background request is consistent with this visibility difference; the rendered live trace identified only initial resize callbacks
between the ordinary and policy stages.

## Fix

Keep a weak per-target initial geometry record using the ordinary pass's existing
local rectangle. On its first resize notification, compare the live x, y, width
and height. An identical first notification does not invalidate the inventory.
A differing first notification, every subsequent resize notification, DOM
mutation, scrolling and window resize retain their invalidation behavior.
Unobserving a retired target removes its baseline; stopping tracking clears the
map. No deadline, request allowance, retry count, policy or key binding changes.

A temporary equivalent filter on the original installed browser changed the
same three live journeys from three failures to three successful publications.
It filtered 177 unchanged initial target notifications and observed no actual
resize in those journeys. Publication observations were 831, 1235 and 876 ms;
these are individual diagnostic samples with polling, not latency guarantees.
The filter was removed after the pilot. No saved site policy or configuration
was changed.
This pilot establishes the cause and effect; it is not a live replay of the new
compiled artifact.

## Regression and artifact checks

The deferred-collection fixture inherited QtTest TestCase's `visible: false`,
which prevented real ResizeObserver delivery. It now renders its WebView.
The new unchanged-initial-size regression fails against the original source and
passes with the fix. Web Animations exercise real size changes without attribute
mutations both before and after the first notification; both still retire the
pending inventory. A removal/readmission case checks baseline cleanup.

| Check | Result |
| --- | --- |
| Visible deferred-collection suite | 54 passed, 0 failed |
| Native document, target and geometry proof suite | 100 passed, 0 failed |
| Release build and linked native test build | Passed |
| Production resource parity, workspace collection, two-tab playback, passive burst, stale refresh and tab receiver cases | 20 passed, 0 failed |

Only the collector script and its regression fixture changed before the build.
The check record verifies exact primary/validation source parity. Reporting
updates after the build are recorded separately. Qualified executable SHA-256:
`85ceb9286b1c0e641036e2a3f991f62f1865ff3d81b028b503b760a92275ab38`.

Raw evidence is under ignored `target/reddit-return-investigation/`:
`native-repro.json`, `native-filter-pilot.json`, observer traces,
`resize-visible-red.log`, the suite logs, `qualification.json` and
`source-inputs.json`. The earlier hidden-fixture attempt is retained separately;
it did not reproduce initial observer delivery and is not the red regression.
The existing extreme-load and wider-platform qualification limits remain as
recorded in the [qualification index](site-policy-qualification.md).
