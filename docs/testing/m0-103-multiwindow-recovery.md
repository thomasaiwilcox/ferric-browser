# M0-103: multi-window session recovery

Requirements: SESSION-001, SESSION-003

Normal profile windows no longer overwrite one shared `current.json` during a
checkpoint. Each Ferric Browser window receives a UUID-backed session identity and
writes its current safe descriptor set to
`current-<session-id>.json` in the profile's session directory. The existing
`current.json` file remains a legacy compatibility input when no window-scoped
snapshot exists.

Recovery enumerates only the exact legacy filename or UUID-shaped window
snapshot filenames. It validates every selected snapshot with the existing
session schema and restore policy, concatenates the selected-tab-first plans,
and sends the combined safe descriptors through the existing lazy restore path.
Malformed input rejects recovery rather than silently producing an empty
session. On a clean startup, before the new run begins writing checkpoints,
the primary window removes stale current-session files; after an unclean
startup, recovery data is retained. Private and ephemeral windows still have
no durable session path.

The adapter unit test verifies UUID filename filtering and the preference for
window snapshots over the legacy file. Workspace tests, the application build,
and native startup smoke cover the integration; restored workspace geometry,
window placement, and richer multi-process session coordination remain open.
