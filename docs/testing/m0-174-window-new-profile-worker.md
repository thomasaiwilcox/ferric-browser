# M0-174: worker-backed `window-new --profile`

`window-new --profile NAME` now queues profile validation on the bounded
profile-list worker. The typed command returns an accepted `profile-lookup`
pending response; Qt creates the `new-window` action only after the worker
response confirms the exact profile name. Missing profiles and registry
errors become status messages without a partial window action.

Commands without an explicit profile retain their existing synchronous
in-memory selection because they do not read the profile registry.
