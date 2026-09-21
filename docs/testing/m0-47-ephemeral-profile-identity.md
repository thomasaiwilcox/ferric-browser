# M0-47 Ephemeral profile identity

Date: 2026-09-16  
Status: in-progress  
Requirements: EPROFILE-001, EPROFILE-002

## Implemented slice

`open --ephemeral` (and the equivalent global `--ephemeral` startup option)
creates a fresh GUI owner with a unique transient profile name and a visible
`Ephemeral · <marker>` label. The reducer represents this as
`PrivacyKind::Ephemeral`, distinct from `PrivacyKind::Private`.

The ephemeral owner uses Qt WebEngine's off-the-record profile and an empty
storage name. Rust profile metadata, history, session checkpoints, contexts,
runtime overrides, and profile locks are not opened for this owner. Recovery is
also disabled for the disposable launch. If an existing normal instance is
running, the request is rejected without changing that instance; cross-window
same-ephemeral-profile invocation tokens are a later slice.

## Verification

Focused static coverage asserts the QML memory-only wiring and CLI tests cover
the new flag. The workspace compiler and native startup smoke remain the gates
for this slice.
