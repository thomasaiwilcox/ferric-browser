# Hint quality repair plan

Users should be able to see a control, identify its hint and select it confidently
while the page continues updating. The repair starts with a proved discovery bug:
slotted containers receive labels despite failing the current selector policy.
It then makes input and labels stable, corrects target descriptions and geometry,
and improves the visual connection between each remaining label and its target.

Status: implementation delivered for installation and testing on 2026-10-14.
The investigation used source `ff9c69b` and the product built from `c9e734a`.
Changes and automated qualification are recorded in the
[repair record](../testing/hint-quality-repair-2026-10-14.md).
Physical Wayland input, screen-reader use and comparative human task measurements
remain manual qualification; they are not inferred from automated checks.
Original findings remain in the
[investigation report](../testing/hint-label-investigation-2026-10-13.md).

## What the evidence changes

A controlled anonymous Reddit experiment compared the production collector with
narrow test-only corrections in the same renderer task:

| Observable result | Current collector | Admission and subtree reuse prototype |
| --- | ---: | ---: |
| Labels | 53 | 28 |
| Candidates that fail their own selector policy | 25 | 0 |
| Coincident rectangle pairs | 23 | 0 |
| Inspection steps | 50,000 | 31,708 |
| Collection truncated | Yes | No |
| Eligible targets lost against completed reference | 0 | 0 |

This is stronger evidence for correcting collection than for broadly removing
ARIA targets, rewriting the selector defaults or adding Reddit-specific rules.
The count is a snapshot, not a target number for every Reddit page. Legitimate
large and overlapping controls still need usable presentation.

Additional reproduced problems are erased input during refresh, label movement,
label reuse and width changes, missing accessible labels, clicking a focus-only
widget without focusing it, and incorrect CSS-scaled iframe coordinates. With
optional `auto_follow=always`, refresh can click a remaining control without new
hint input. The default `full-match` setting did not exhibit that automatic click.

## Intended user experience

Pressing `f` shows eligible controls with compact labels close to their visible
content. Typing retains the existing alphabet and full-match behavior. Tab and
Shift+Tab preview targets; `/` filters by text; Escape cancels. Routine page updates
preserve the user's partial input, selected target and surviving label positions.
A removed or repurposed target cannot silently become a different action under
the label the user was reading.

The keyboard-selected target gets one clear outline and a browser-owned preview
such as `af · Button · View comments`. The action description comes from the same
Rust policy that resolves selection. A passive hover description keeps the keyboard
selection visible; typing dismisses that description. Hover previews do not focus,
hover or click the webpage and do not replace the keyboard selection. Names render as plain
text; a colour change is never the only indication of state or target type.

Most controls retain a single short badge. If valid targets cannot fit without
ambiguity, show a compact stack with a count. Tab can reach every member; the
existing collision command cycles the visible member and its preview. Each
member retains its own label and action. An expanded local list shows labels and
names, and typing any member's full label still selects that exact candidate.
Opening or cycling the stack never activates a representative on the user's behalf.

Scroll targets have a named scroll affordance. The document root can use a small
edge badge labelled `Page scroll`, rather than an unexplained letter at a page
corner. Keep explicit `scroll-target select`, Region and Surface workflows.
Genuine collection limits produce concise, actionable status, once per relevant
state change; repeated refresh must not flood notices.

## 1 Correct admission and reuse completed discovery work

Primary implementation: `qml/scripts/PageInteractionScripts.js`, through the
existing `BrowserScripts.js` facade and the existing retained element map.

- Apply the same eligibility predicate to ordinary traversal, assigned roots and
  other discovery entry points. Walking through a container must not make that
  container a candidate. Preserve the distinct, checked admission rules for
  scrollables, facets and opt-in proxies, and selector `extend`/`replace` behavior.
- Require every published candidate to satisfy the captured family policy that
  final validation will use. Add behavioral coverage for unchanged DOM, rather
  than weakening the final validator to accept the extra containers.
- Reuse completed physical subtree scans within one synchronous collection.
  Before skipping a slotted subtree, establish that its nodes were processed and
  its permitted shadow roots, slots and frames were scheduled for this same
  scope/context. Track pending and completed work explicitly where necessary.
- Do not use an unconditional visited-root set. A tested variant lost a valid
  target because its first traversal reached a depth limit. Preserve boundary
  depth, frame path, scope containment, clipping and discovery-family provenance;
  handle shallower later paths without dropping previously unvisited boundaries.
- Remove repeated preferred-link subtree scans using collection-local scope/link
  facts. Compute candidates and same-destination preference from the bounded
  discovered set, caching visibility reads for the current pass. Preserve the
  existing preference behavior through equivalence tests; do not turn matching
  URLs or rectangles into a universal deduplication rule.
- Keep the 5,000-candidate and 50,000-inspection limits. Account for auxiliary work
  and bounded storage, including scope scans and frame indexing. Keep incomplete
  results explicitly marked when any required phase cannot complete.

Start with the narrow admission repair, followed by traversal and link-preference
work in separable changes. A revised collector must retain legitimate slotted
buttons, shadow controls and scoped descendants, not merely produce fewer labels.

## 2 Reconcile refreshes within the current user interaction

Primary implementation: core `hints.rs`, engine `browser_ui_navigation.rs`, and
`FerricBrowserRuntimePresentation.qml`.

Introduce an explicit refresh transition in `HintInteraction`. Preserve its input
mode, prefix, query and selected identity when that target's action meaning is
unchanged. Recompute matches against the fresh validated snapshot without invoking
initial activation or replaying input events. Target Search should use the same
reconciliation rules where applicable, retaining its explicit confirmation rule.

Separate initial entry, passive refresh and rapid-mode continuation. Initial
single-target activation under `always` remains an entry behavior. A passive
refresh never produces an activation effect. The existing native-pointer
confirmation and palette permits continue to invalidate at their current authority
boundaries; preserving visual state must not preserve an obsolete permit.

Strengthen the label lifetime contract:

- Keep fixed width throughout an interaction, including rapid continuation.
- Preserve bindings by owner/document/frame/element/related element and validated
  action meaning. Compare meaning exactly in Rust; do not use a presentation hash
  as an authorization check. Include destination and intent; for non-URL controls,
  conservatively invalidate on a changed bounded action name. Do not claim this
  detects arbitrary changes to page JavaScript handlers.
- Retire labels when targets disappear, are consumed or change meaning. Do not
  give those labels to a different target during the same interaction. A temporarily
  offscreen target may regain its reserved binding only with the same identity and
  meaning; consumed or repurposed targets must not regain an actionable old binding.
- Allocate unused labels to new candidates only while the current prefix-free
  space and bounded session storage permit it. If capacity is exhausted, retain
  current usable hints and show that restarting hints will include new controls.
  Do not silently widen or reassign every label while the user is reading it.
- If the selected target disappears or changes meaning, clear selection and require
  a new explicit selection. Enter must not fall through to a newly chosen first row.

Keep this state with the existing Hint interaction/session owner. Continue issuing
fresh snapshot/session identifiers where the action-permit machinery requires
it. Do not add a second DOM identity registry or a second text matcher.

## 3 Correct the facts used by preview and activation

Primary implementation: `PageInteractionScripts.js`, core `HintKind`/intent policy,
`hint_payload.rs`, Hint selection, and Target Search's existing kind mapping.

Use one bounded label extractor for ordinary, facet and proxy targets. Support
`aria-labelledby`, `aria-label`, native associated labels, image alternative text,
appropriate visible control text and title fallback. Resolve references within
permitted tree roots and bound reference count, traversal, characters and UTF-8
bytes. Exclude editable contents and input values from generic fallback text.
Use localized kind-based fallback names when no safe name exists.

The [W3C accessible-name algorithm](https://www.w3.org/TR/accname-1.2/) is the
reference for name precedence and relationships. This privacy-bounded helper is
not a claim to implement the entire accessibility-name algorithm. Tests must
include icon-only controls, shadow-local references, missing IDs, cycles, hidden
referenced labels and hostile markup/control characters.

Distinguish a legitimate focus-only target from a clickable ARIA widget. Add a
narrow typed kind for built-in candidates discovered solely through focusability;
Rust resolves it to exact DOM focus, and the final helper verifies actual focus.
Do not change all `Aria` candidates to focus: role buttons, switches and custom
click targets still require their own semantics. Preserve explicit selector
policies and document this intentional improvement to default focus-only behavior.

Add bounded presentation geometry where needed: a visible fragment/anchor and a
small capped set of outline rectangles. Validate finite values, containment and
size bounds in Rust. These fields describe presentation; the native pointer
adapter still obtains its own freshly validated activation point.

Use one tested frame projection path for collection and freshness. Correct
axis-aligned scale/translation, borders, clipping and nested frames. Detect
unsupported rotation, perspective or ambiguous mappings and omit inaccurate
child hints, retaining the eligible frame exterior where applicable. Do not
expand native-pointer transform support as an incidental visual fix.

## 4 Anchor labels and expose their meaning

Primary implementation: `FerricHintOverlay.qml`, runtime presentation/chrome and
the existing Hint footer. Use measured text dimensions for configured alphabets,
font scaling and marker sizes.

Choose a canonical anchor on an actual visible fragment. Reserve surviving
placements before placing new labels. Cache placement by interaction owner and
candidate identity, invalidate on the appropriate lifecycle boundaries, and move
a marker with its target when geometry changes. Filtering should not move the
remaining labels to fill holes.

Set a small displacement budget, initially one marker height plus four logical
pixels from the canonical anchor, to be qualified visually. Do not jump from one
corner of a large card to another merely to avoid a neighbour. If a badge must
leave its fragment, draw a short connector with a clear endpoint. If no unambiguous
local placement fits, make the collision explicit with a stack.

Build collision groups from actual marker footprints and anchor proximity after
coordinate mapping. Avoid grouping a viewport-sized scroll root with every
control it contains. Cycle the presented group, replacing the current mismatch
between target-rectangle intersections and marker collisions. Remove or implement
the unused `collisionRotation` property as part of that change.

Keep one selection preview and at most one passive hover description, rather than
constructing a full detail component for every candidate. Reuse browser palette,
contrast, scale and accessibility facilities.
Expose label, name, role, action and stack position to accessibility without an
announcement on every page mutation. Preview must be usable with keyboard alone;
mouse hover is an additional affordance. Preserve direct selection and the existing
validated action palette for each exact target.

The [Vimium implementation](https://github.com/philc/vimium/blob/master/content_scripts/link_hints.js)
provides a useful comparison for cycling overlapping marker rectangles. Reuse the
principle, not its implementation or complexity: Ferric already has a spatial
index and different native presentation and authority boundaries.

## 5 Prove the repair across the full workflow

Promote the temporary characterization cases into permanent contract tests that
assert the desired repaired behavior. Register them in the existing test matrix
and use the production collector, Rust state, QML presentation and application
entry paths at the appropriate level.

| Case | Required result |
| --- | --- |
| Plain slotted content in `all`, `links`, `buttons`, custom replace/extend | No label unless that family actually admits it; eligible children remain reachable |
| Nested and flattened slots, revisited roots, shallower later paths | Bounded traversal; no loss relative to a completed fixture reference |
| Scope rooted inside shadow content, slotted descendants, empty/replaced scope | Exact scope only; no whole-page fallback or retained stale ownership |
| 250 sibling links plus visible shadow button | Complete result within the existing budget; final button reachable |
| Distinct co-located handlers | Both choices remain reachable and invoke only their selected action |
| Unrelated mutation during partial label, `/` filter or Tab selection | Input and surviving selection persist; no background effect under any auto-follow policy |
| Removal, replacement, changed destination, 9/10 and 81/82 boundaries | No label reuse, shrink/reassignment or silent action substitution |
| Stationary card with a new neighbouring target | Surviving marker position stays fixed, or an explicit local collision is presented |
| Dense tiny controls and wrapped links | No unconnected marker presented closer to the wrong control; fragment/connector identifies its target |
| Zoom 50/80/100/125/200 percent, DPR 1/2, edges and scaled frames | Correct mapped anchors and clipping; unsupported mappings are explicit |
| Icon names, focus-only controls, scroll root | Useful names and correct focus/scroll effects; no editable-value exposure |
| Tab/mode/config changes, pending action, renderer loss, rapid session | Correct state teardown, stale-effect rejection and existing confirmation limits |
| Accessibility, large marker scale, keyboard-only stack traversal | Target and action understandable without colour or pointer hover |
| Mouse movement while a keyboard target is selected | Keyboard target remains visibly selected; Enter acts on it, not the hover description |

Run core/engine tests, the Qt component and WebEngine suites, production application
journeys, formatting, strict Clippy, workspace/CLI checks and applicable generated
resource/contract checks. Include the existing pointer and scope integration tests
because Hint identity and payload changes affect those paths.

Keep the established performance gate: p95 at most 200 ms for 500 visible
candidates at 80/100/125 percent zoom, with 10 warm-ups and 100 measured samples.
Investigate a same-host common-case regression exceeding both 10 percent and
10 ms. Record 1,000/5,000-target and high-mutation cases separately, including
layout-to-visible latency. Run the existing 1,000-cycle lifetime check with the new
placement and retired-label state. No unbounded cache or extra per-refresh delegate
retention is acceptable.

Finally qualify actual user tasks in a disposable installed profile: identify and
focus search, open a post, identify a vote control, open comments and a menu, and
select the intended scroll area. Use owned fixtures for state-changing actions.
Cover Reddit, an ordinary document page and dense application controls. Record
physical Wayland input separately from offscreen Qt input and anonymous page-script
results. Compare task completion, wrong-target attempts and time to identify the
intended control; treat usability gains as measured results, not inferred from a
lower hint count.

## Delivery order and completion rule

1. Land deterministic regressions and the admission repair.
2. Land bounded traversal and link-scope reuse with target-set equivalence checks.
3. Land refresh reconciliation and stronger label lifetime semantics.
4. Land naming, focus intent and geometry corrections through typed boundaries.
5. Land anchored presentation, previews and actual-marker collision handling.
6. Build in isolation and complete integrated, performance, accessibility and native
   validation; update user guidance and release evidence. Install the qualified
   artifact, then verify the installed binary and smoke-test it in a disposable profile.

Each change should have its own behavior and evidence. The collection correction
should be independently reviewable before the presentation changes. Keep the
existing `f` command, alphabet, families, custom-selector mechanism and native
activation safeguards. No site-specific exception, new label-generation strategy,
new target registry or new search engine is required.

Completion requires accurate and understandable targets, stable interaction during
page updates, no passive-refresh activation, preserved access to legitimate
controls, bounded performance and successful end-to-end selection. A screenshot
that merely looks less crowded does not satisfy these gates.
