# ADR: exact Hyprland identity for test-owned page windows

**Status:** native proof qualified on the pinned stack; production grouping remains unqualified
**Scope:** W00 candidate only. This does not authorize production grouping.

## Decision

The proof fixture assigns a fresh, opaque xdg-toplevel tag to each owned
`QWindow` before mapping. It resolves a window only when the running Hyprland
instance reports exactly one matching tag. The fixture then corroborates the
observed PID and app ID and re-resolves the compositor `stable_id` and address
before any operation. A stale instance, missing marker, duplicate marker,
changed stable ID, or inconsistent address is a rejection.

The tag is an ephemeral correlation token. It is not a page title, persistent
browser identity, or permission to act on a window. Test output records only
case results and aggregate booleans; it does not save the compositor client
list, titles, or unrelated window identifiers.

Hyprland's public Lua `Group:add(window, index)` API is the placement candidate
for an already-created source group. The fixture checks that the source group
is present, allowed and unlocked, and that the destination is standalone. It
captures the source group's current member and index, adds the exact resolved
destination, then requests restoration of the prior current member. A target
that is already grouped is rejected before calling `add`, because the installed
implementation merges its whole source group. The proof also watches the
Hyprland active-window event stream and fails if placement changes focus.
The observer canonicalizes the event stream's bare hexadecimal addresses to
the snapshot's `0x` form. Only focus events extend its bounded settling
interval; unrelated desktop events do not prevent observation from completing.

## Failure behavior

Unknown compositor version, unavailable Lua evaluation, missing xdg tag,
ambiguous identity, stale identity, unsupported grouping, locked or denied
group, disconnected IPC, or failed postcondition fails the native proof. No
address, title, app ID, PID, active-window order, or directional guess is used
as a substitute for the tag plus stable identity. The fixture does not unlock
pre-existing groups or change compositor configuration.

## Qualification boundary

T100–T102 are intended to execute against the exact installed Hyprland build
using disposable windows from the test fixture. A C++/Lua source path or
standalone Qt window test cannot qualify actual native mapping or grouping.
Qualification requires captured receipts for the positive placement,
same-process and delayed-map identity, stale and duplicate rejection, exact
membership, and background-focus checks. Address reuse is recorded only if
the compositor actually reuses an address; a non-occurrence is not reported as
an exercised reuse case. No production transport, user-window behavior,
workspace policy, or cross-compositor support is claimed here.

On 2026-10-26, T100–T102 passed on Qt 6.11.2, matching KWindowSystem
6.28.0, and Hyprland 0.56.2 commit
`efb50993780079460b0cbed1363e2166a2de1d9f`, using disposable live-session
windows. Actual address reuse was observed. Background placement preserved
both the active window and the group's current member, with zero focus events.
Locked and missing sources were rejected without changing focus. The version
admission gate also rejected an intentionally incompatible build requirement;
this is not a test on a different compositor backend.

The earlier session focus precondition failure ceased without desktop
configuration changes; its exact cause remains unconfirmed. Subsequent test
failures exposed the observer's unrelated-event timer reset and address-format
mismatch, repaired as described above. Historical failed receipts are retained
alongside the W00 attempt-3 qualification record in the external run workspace.
