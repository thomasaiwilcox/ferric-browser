# S26 working plan — wheel-only task gap

## Patch outline

This is a research-only static review. No wheel transport or production code change is planned. Compare the recorded wheel/local-scroll requirement with the current page movement commands, Auto/Document/exact Element scroll policy, scoped Set scroll target action, and the documented map/diagram task. Distinguish scrolling from application-defined wheel zoom/pan and from S20's separately gated pointer drag.

## Test-to-path map

| Test | Evidence path |
| --- | --- |
| T037 — scoped hints and scroll independence | The current engine gate in `S25-engine.log` includes `HintGeometry::test_body_overflow_propagates_to_viewport` (select exact nested pane, invoke the existing `scroll` operation, observe `scrollTop` increase), `WebAppInteraction::test_T037_scroll_target_keeps_existing_weak_setter`, and the empty/replaced-scope no-fallback case. The S14 result also records no-focus/no-scroll behavior. |
| T054 — wheel-only task evaluation | `S26-static-inspection.log` records the C03/C14/C19/source-task and production-command scan. The report identifies the only documented custom-surface example and checks it against existing page shortcuts and scroll-target actions. |

## Decision threshold

Recommend `no_need` only if no concrete documented task remains after testing the existing page shortcuts and explicit target path. A hypothetical site-specific wheel-zoom/pan interaction, without an authorized live task or a fixture demonstrating the gap, does not justify a second global scroll policy. If such a task is later evidenced, it needs a separate bounded native-input ADR with an exact local owner/scope, explicit user intent, bounded delta/modifiers/duration, lifecycle cleanup and no page-JavaScript wheel synthesis.
