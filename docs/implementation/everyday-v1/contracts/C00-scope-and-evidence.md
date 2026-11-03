# C00 — Scope, authority and honest completion

## Required deliverables

Implement hot-page preparation, phone passkeys and optional native page windows with optional Hyprland grouping. They are independent switches and independently qualified capabilities. Internal browsing is the regression control. The task is implementation, not merely architecture documents or a set of disabled placeholders.

Defaults are internal tabs, no hot spare, phone provider off, grouping off. The user may opt into a capability only after its admission proof is accepted. Runtime missing prerequisites still disable that capability regardless of an `on` setting. A capability may be implemented but unqualified. Do not equate “default off” with “not implemented”; equally, do not call an all-rejection stub an implementation.

No predictive web prefetch, daemon/autostart, private prewarming, live presentation switching, password/passkey vault, credential synchronization, remembered phone, conditional autofill, related-origin expansion or general iframe phone support. Do not expand into the user’s 2.0 browsing/versioning work.

## Evidence levels

Distinguish source inspection, pure unit test, component test, composed production application, nested/native Wayland, actual compositor-delivered input, benchmark and physical authenticator. An offscreen Qt key event does not prove physical keyboard routing. A mock phone does not prove Bluetooth/phone compatibility. A generated QR does not prove a valid WebAuthn signature. A passing source-string test only proves the string is present.

Every result identifies the exact source commit/tree, build mode, dependency/SDK identity, command, exit status, assertion/test count, environment, evidence path and limitations. A skipped, missing, timed-out or zero-test command is not a pass. A browser fixture must exercise the production bridge/host, not a separately reimplemented model of it. Shared schema tests are useful but not sufficient for integration.

## Gate failure

If a required API proof fails, retain tests reproducing it and a disabled capability; record missing primitive and examined safe alternatives. Do not ship a fallback that trusts page origin, invents compositor identity or removes isolation. Continue independent tasks. `blocked_environment`, `blocked_platform`, `blocked_specification`, `implemented_unqualified` and `failed` remain visibly distinct from `accepted`.

The coordinator may split a task, update a renamed source anchor or register tests through a recorded plan amendment. It may not waive a security invariant, claimed user behaviour, threshold or physical qualification to make progress appear complete. Any numeric budget change requires evidence and explicit approval.

A release report must distinguish full completion from an implemented subset. Never claim all three features complete while phone/device or Hyprland positive-path qualification is outstanding.
