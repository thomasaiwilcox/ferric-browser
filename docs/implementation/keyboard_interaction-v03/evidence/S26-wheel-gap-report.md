# S26 — Wheel-only task gap evaluation

## Decision

`no_need` for the current v0.3 pack. No documented, reproduced task requires a new wheel mode after using existing page movement commands and the exact scroll-target policy. This decision does not claim universal support for map, canvas or custom-widget wheel semantics. Production scrolling behavior is unchanged.

## Existing path and T037 evidence

Normal mode already provides `Ctrl+f`/`Ctrl+b` for viewport-page movement, `Ctrl+d`/`Ctrl+u` for half-page movement, and `gg`/`G` for top/bottom. A focused core test passed for the half-page and `;s`/`;S` bindings. `scroll` supports directional movement, while `scroll-page` and `scroll-to` provide page and edge operations. `;s` enters Hint selection for a scroll target; `;S` restores Auto. The existing resolver distinguishes Auto, Document and a selected element, uses the target's active axis and makes unsupported axes a no-op.

The element path preserves exact identity: Hint collection retains the target record; final selection rechecks frame/document reachability, meaningful visibility and x/y scrollability; the policy stores weak references to the exact element and owner document. It does not focus the page or use a guessed replacement. S25's fresh engine log includes `HintGeometry::test_body_overflow_propagates_to_viewport`: the fixture selects a nested scroll pane, invokes `BrowserScripts.scroll("scroll", "down", false, 1)`, and observes that pane's `scrollTop` increase. It also includes T037's scoped scroll-target case, which checks the exact weakly retained element, and the empty/replaced-scope no-fallback case. S14's T037 run separately records that stale scopes do not widen to the page and scoped interaction preserves focus and scroll.

That path addresses the documented multi-pane reading/output case. The source's diagram/map example selects a visual surface for scoped Grid click/hover and gates later pan/drag on native gesture qualification; it does not require a wheel action. Maps are listed as benchmark targets, not as proof of a current Ferric wheel-only defect. S18 records that no authorised live-site matrix was available, so no real map or slider interaction is claimed here.

The native adapter observes physical `QEvent::Wheel` only while scoped Grid is visible or a local Hint pointer action is pending. In that transient state it invalidates the pending native gesture, consistent with physical input taking ownership; outside that state it does not install a global browser wheel policy. This does not synthesize wheel events or redefine the exact scroll target.

## What would reopen the question

A separate investigation would need a specific authorised map, canvas or custom control task showing that its required wheel effect is neither page scrolling nor a page shortcut and cannot be reached through the existing scroll target. It would then need to record whether wheel means zoom, pan, value adjustment or another app-specific action. Only that evidence could justify a separate bounded native-input ADR with an exact local owner and scope, explicit intent, constrained deltas/modifiers/duration and lifecycle cancellation. Page-JavaScript wheel synthesis, an arbitrary IPC coordinate route and reuse of the selected DOM scroll target remain outside the decision.

No new wheel mode, default key, scroll policy, or native transport was added. The current result is a scoped no-need finding for the v0.3 evidence available in this checkout.
