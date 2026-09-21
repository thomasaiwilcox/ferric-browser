# M0-58 — Journey traversal and branching

Back and forward are deferred graph operations. The reducer records the
requested offset, leaves the current journey node unchanged while the engine
is loading, and moves the current node only when the matching navigation
commit arrives.

A normal committed navigation after moving back truncates the tab's forward
path and receives the typed `branch-after-back` transition. The abandoned
node remains in the graph while it is retained, so the journey outline can
show where the branch diverged.

Durable normal-profile storage mirrors this behavior by updating
`journey_current` on a traversal commit instead of inserting a new journey
node or visit. Private and ephemeral graphs remain memory-only.
