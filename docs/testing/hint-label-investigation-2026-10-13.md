# Hint label clarity investigation on Reddit

The deeper investigation identifies an eligibility bug as the main cause of
Reddit's excess labels: slotted content roots bypass selector matching. In a
controlled anonymous snapshot, 25 of 53 candidates failed the collector's own
policy check. Correcting admission removed all 23 coincident rectangle pairs.
Repeated slotted-subtree traversal also exhausted the collection budget.
Independent presentation, refresh-state, naming and frame-geometry defects remain.
Basic zoom and display-scale mapping passed the existing geometry cases; a new
CSS-scaled iframe fixture fails correct projection.

The proposed repair and acceptance criteria are in the
[hint quality plan](../implementation/hint-quality-plan-2026-10-13.md).

Source reviewed: `ff9c69b`, with installed product source `c9e734a`. This is a
self-review and diagnosis. Product source and the installation were not changed.
Reddit was identified by the user. Live measurements used a fresh anonymous,
off-the-record WebEngine profile at 1000 by 800 logical pixels. Only target kinds,
rectangles and aggregate counts left the renderer; account/feed text, URLs and
input values were not included in the measurements.

## Reddit observations

The production collector found 53 candidates: 25 generic ARIA/focusable targets,
15 links, ten buttons, one scrollable, one textarea and one media target. There
were 23 pairs with rectangles matching to within one CSS pixel and seven target
rectangles larger than 20 percent of the viewport area. These are geometric pairs,
not proof that the underlying elements have identical behavior.

Using the default alphabet and two-character label width, the production overlay
layout placed 12 marker centres outside their own target rectangles. An outside
centre alone can be a necessary consequence of a small control. Without a visible
connection or preview, however, the user has little evidence of the intended
target. This probe matches default label width and y/x reading order; it does not
reproduce the user's account, configured alphabet, zoom or exact tie ordering.

Collection returned `truncated: true` after 50,000 inspection steps. That counter
includes repeated subtree walks used to prefer smaller same-destination links,
as well as the main DOM walk. It is not a measurement of 50,000 unique elements.
The cap bounds counted work, but completeness of the target set is unproven. Raising
the cap would not correct ambiguous placement and would increase collection work.

The user's subsequent Reddit screenshot corroborates the presentation problem:
several labels surround the search field and individual post-action controls,
while other labels appear in blank areas and at page corners. Those positions
are consistent with broad container targets being included alongside their child
controls; the screenshot alone cannot identify the exact DOM element behind each
label. Its status bar also confirms truncated collection on the user's page.
The pattern does not establish a uniform coordinate offset. No account details,
feed text or copy of the supplied screenshot were added to the repository.

The [anonymous probe log](../../target/hint-investigation/reddit-geometry-default-labels.log)
and [probe source](../../target/hint-investigation/reddit/tst_reddit_geometry.qml)
record the snapshot. The earlier numeric-label trial used wider three-character
test labels; its placement count is superseded by the default-label trial above.

## Reproduced presentation defects

### P2 Labels can look attached to a neighbouring control

[FerricHintOverlay.qml](../../crates/ferric-browser-engine-qt/qml/components/FerricHintOverlay.qml)
tries target corners, above/below positions and small offsets to avoid already
placed labels. It does not score whether a marker looks closer to another target,
and the marker has no connector to its target. Only the active target has an
outline. Target names are provided to accessibility but are not displayed as
hover descriptions or previews.

A fixture rendered the actual component over 18 small controls and two co-located
large targets. Four small-control marker centres were closer to a neighbouring
control's centre than to their own. The
[rendered example](../../target/hint-investigation/dense-hints.png) shows labels
alternating above, over and below the rows. This is a reproducible visual
association defect; the stored label still identifies its original target.

### P2 A fixed target's label can jump on refresh

Layout is recomputed greedily from the current candidate order, with no retained
marker placement. Adding a small control above an unchanged card moved its
`da` marker from `(340, 100)` to `(542, 100)`: 202 logical pixels horizontally.
The card and its label identity did not change. Rust preserves labels when label
width stays constant, but that does not preserve their displayed positions.

The collision-control property is also unused by the overlay. The runtime's
collision cycling groups intersecting target rectangles, rather than the marker
rectangles produced by layout. It therefore does not cover every visual collision
between labels belonging to separate controls.

### P2 Multiple target meanings share one visual label style

The real collector emits both an outer role-button wrapper and its inner button
when they occupy the same rectangle. A deterministic WebEngine fixture reproduced
two candidates for one visible button footprint. It also collected the document
scroll root, whose rectangle spans almost the whole viewport. Both behaviors are
allowed by the current discovery policy, but the presentation offers the same
letter-only marker for links, buttons, focus targets and scroll targets.

Reddit's coincident and large rectangles make that weakness visible. Deduplicating
solely by rectangle would be unsafe: two overlapping elements may have distinct
actions. Grouping them for presentation must preserve each exact element identity
and its existing action eligibility checks.

## Verification and coverage gap

The characterization probes passed seven Qt test rows, including setup/cleanup.
They call the production overlay and collector; they characterize existing
behavior rather than assert that the presentation is acceptable. The anonymous
Reddit probe passed three rows. Existing HintGeometry cases passed nine rows at
DPR 1 and nine at DPR 2, including 50, 100 and 200 percent page zoom, clipping and
an untransformed same-origin frame.

The current overlay performance checks verify viewport containment, retained
labels and non-overlap for unclustered markers. Those properties can hold while
labels remain ambiguous or jump across an unchanged target. The geometry tests
do not establish visual usability on every Reddit layout or transformed frame.

All probe artifacts are under `target/hint-investigation/`. The probes record no
live feed screenshot or account data. The rendered example is a synthetic control
fixture using the real overlay component; the separately supplied user screenshot
was used only to corroborate the visible layout and truncation status.

## Deeper investigation

### Slotted roots bypass eligibility

`visit()` seeds its `normal` list with an assigned root whenever `includeRoot` is
true, without checking the requested selectors. A visible plain `div` assigned
to an open shadow root's slot is therefore emitted as an `aria` candidate in both
`all` and `buttons`. The production fresh validator immediately reports it as
not visible/eligible, and the final click returns false. The misleading label is
not evidence of a final action-authority bypass: validation correctly rejects it.

This explains the broad non-interactive containers in the Reddit measurements.
It supersedes the earlier hypothesis that all of those containers were legitimate
focusable candidates that merely needed a different presentation.

### Redundant traversal dominates Reddit's inspection budget

An instrumented anonymous pass counted 49,639 ordinary traversal inspections and
361 preferred-link inspections. Traversal reached 15,818 distinct nodes through
1,662 root visits, including 965 repeated roots. Preferred-link work is a separate
problem, but it was not the dominant cost on this Reddit snapshot.

A deterministic fixture with 250 links and 258 elements exhausted the same
50,000-step cap: 49,491 steps were preferred-link checks. Its visible shadow-root
button was omitted from `all`, while collecting `buttons` alone found it.
Both duplicate subtree traversal and repeated link-scope scans need repair.

### Controlled collection experiment

A test-only source transform ran variants of the production collector in one
renderer task on the same anonymous Reddit DOM. It returned aggregate facts only.
The reference variant applied the admission fix and used a larger test-only work
budget to complete traversal. This is a comparison oracle, not a proposed limit.

| Measurement | Production baseline | Admission fix | Admission plus completed slotted-subtree reuse |
| --- | ---: | ---: | ---: |
| Candidates | 53 | 28 | 28 |
| Candidates failing their own selector policy | 25 | 0 | 0 |
| Coincident rectangle pairs | 23 | 0 | 0 |
| Large targets exceeding 20 percent of viewport | 7 | 3 | 3 |
| Empty names | 10 | 2 | 2 |
| Counted inspection steps | 50,000 | 50,000 | 31,708 |
| Truncated | Yes | Yes | No |
| Missing eligible targets versus completed reference | 0 | 0 | 0 |

The remaining three large targets were legitimate under existing discovery
policy. Fewer labels here came from correcting admission, not arbitrary ranking
or hiding valid controls. The baseline's truncation did not omit an eligible
visible target in this particular snapshot; it does omit one in the deterministic
fixture above. The warning alone cannot establish which visible targets are missing.

A naive visited-root cache was also tested and rejected: it stopped truncation
but lost one valid media target. A root can first be reached through a deeper
traversal path. An implementation must preserve boundary discovery, scope and
depth semantics when reusing completed work. The successful narrow experiment
skips a slotted subtree whose physical nodes were already scanned by the same
collection. It remains a prototype pending permanent scope and lifecycle tests.

Single-pass elapsed times are retained in the logs, but variant order and cache
warming prevent treating them as a benchmark. The successful prototype took about
368 ms in this run; production performance qualification remains necessary.

### Refresh replaces user input and can trigger an action

A temporary application harness linked the existing production engine and loaded
production QML. Real QTest key input entered a partial label. An unrelated body
attribute mutation caused refresh to erase the prefix. The same experiment with
the ordinary `/` text filter erased the query and returned to label mode.
`begin_hint_session()` preserves a query only for Target Search and otherwise
constructs a new `HintInteraction` on every snapshot.

With the optional `hints.auto_follow=always` setting, removing one of two buttons
caused the remaining button to be clicked without further hint input. Refresh
re-runs `initial_activation()`. The default `full-match` setting was not shown to
perform this automatic click. Initial single-target auto-follow and background
refresh must become distinct transitions.

### Label identity has additional instability

A standalone probe called the production core APIs. Reducing ten candidates to
nine changed all nine surviving labels from two characters to one. Replacing an
element recycled its old label for a different element. Changing a retained
link's destination kept its label. These follow the current algorithm; stronger
session stability must cover label width, retired labels and changed action
meaning, in addition to the earlier QML marker-position defect.

### Missing names, focus semantics and transformed frames

- An icon button named through `aria-labelledby` and an image named through `alt`
  both produced empty names. A control using `aria-label` worked. Ordinary,
  facet and proxy labels currently use different extraction rules.
- A plain `tabindex=0` keyboard widget was classified as `aria`. The production
  click returned true while focus stayed outside the widget. Core maps that kind
  to semantic click, not focus. This is a separate legitimate focus target, unlike
  the ineligible slotted containers.
- A same-origin iframe scaled by CSS `transform: scale(2)` produced a child hint
  at `(70,60,80,20)` instead of `(100,80,160,40)`. Projection adds frame offsets
  but does not apply scale. This establishes an additional geometry defect;
  it is not established as a cause of the user's Reddit screenshot.
- Two co-located legitimate controls fired distinct handlers in a synthetic
  parent/child fixture. Rectangle-only deduplication would remove a real action.

### Evidence and limits

New artifacts are under `target/hint-investigation/deep/`:

- `qml-complete.log`: six production-script characterization cases, eight rows
  including setup and cleanup.
- `application.log`: two production-application characterization cases, four
  rows including setup and cleanup; Qt events in offscreen mode.
- `core.log`: three assertions through production Rust label APIs.
- `reddit-traversal.log`: live anonymous traversal and candidate structure.
- `counterfactual-slot-subtrees.log`: controlled collection comparison.
- `counterfactual-depth.log`: rejected root-cache experiments and reference.
- `validation.json`: source identity, commands, results and artifact hashes.

Passing characterization cases prove the reported behavior, including defects;
they are not green acceptance tests for a repaired browser. The application
runner emitted offscreen graphics and teardown warnings, retained in its log.
No physical keyboard qualification or installed-browser repair was performed.
No authenticated page, account details, feed text, input values or live screenshot
was saved by these probes.

## Recommended correction

Repair candidate admission and duplicate traversal first. Preserve eligible
controls and the existing work limits, with separate regression cases for
slotted roots, completed subtree reuse and repeated preferred-link work.

Then reconcile snapshots within one user interaction: retain input, selection,
labels and placement when their target meaning survives; retire invalidated
bindings; prohibit background-refresh activation. Render anchored markers with
clear target previews and small, explicit stacks only when valid targets still
collide. Correct accessible labels, focus-only intent and supported frame mapping
through the existing typed boundaries. The implementation plan specifies the
behavioral decisions, ownership, tests and release gates.

For temporary relief, `:hint links` or `:hint buttons` can reduce clutter, but the
slotted-root admission bug affects those families too. These commands are not a
reliable workaround for the underlying defect.
