# C11 — Observation, focus and degraded operation

Use a bounded event-driven connection with snapshot/resynchronization, capped responses, coalescing and backoff. On reconnect, increment compositor instance/connection epoch and invalidate old native mappings and pending operations. Current grouping/order/focus is observed from the compositor. Ferric does not run a loop restoring its preferred grouping against user choices.

Only binding records proven by C10 are actionable. Validate host generation, native client still present, source and target same allowed profile/privacy/context scope, backend capability and lock state immediately before effects. Treat app/PID as corroboration, not unique identity. Address reuse, events racing snapshots and windows disappearing mid-request have dedicated tests.

Background opens must not focus, raise or activate a different page. Foreground opens may request activation only as part of the originating explicit action, following the compositor's normal policy. Rejection must not trigger an activation retry loop. Test handoff while the user changes workspace/window. Keep a per-operation outcome, not an optimistic “grouped” flag.

Mixed groups can contain other applications or differently scoped Ferric pages because users can regroup manually. Browser traversal/close/move operates only on eligible registered Ferric page identities. Never count a foreign app as a tab or remove it while restoring a session. Pinned browser pages remain distinct from compositor pinned/sticky state.

Track mapping and known visible/occluded facts separately from keyboard focus and current internal index. An unfocused tiled page remains potentially visible. Unknown occlusion keeps the renderer active under current lifecycle policy. Media/capture/dirty forms/active authentication and all existing suppression conditions remain.

When Hyprland IPC fails but the display connection remains healthy, keep all page hosts usable and use generic window navigation/fallback. If the compositor/display server itself dies, use existing application crash recovery; do not promise live Qt windows survive a broken display connection. Never rewrite compositor configuration as recovery.

`hyprland.page_grouping = off | auto` extends the existing root `hyprland` namespace. It is separate from existing workspace-routing permission. A user may enable grouping without authorizing arbitrary workspace moves. `enabled=off` suppresses all new compositor actions. Auto detects capabilities; it is not permission to coerce missing ones.
