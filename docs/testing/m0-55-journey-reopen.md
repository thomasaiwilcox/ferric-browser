# M0-55 — Safe journey node reopening

Normal profiles can reopen an existing durable journey node with:

```text
journey-reopen NODE_UUID [--target current|tab|window]
```

The command looks up the node by UUID in the active profile database, refuses
missing/pruned nodes, rechecks the stored URL safety policy, and starts a fresh
GET navigation. It never accepts a caller-supplied URL, POST body, renderer
state, authentication state, or form data. The commit path uses a typed
`reopen` transition with the source category `journey-reopen`.

The native journey manager exposes the validated `current`, `tab`, and `window`
target choices before invoking the same command path. Private and ephemeral
in-memory recovery remains limited to the current window or a new tab within
that transient owner.

`window` targets create a new normal-profile browser window through the native
window action.
