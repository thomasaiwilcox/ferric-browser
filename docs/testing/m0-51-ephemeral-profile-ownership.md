# M0-51: Ephemeral profile ownership across windows

## Scope

This slice gives live ephemeral windows an explicit invocation token, records
their WebEngine profile in the in-process window registry, and counts each live
owner of a shared token.

## Evidence

- `Main.qml` registers the primary and secondary window with its profile,
  transient marker, and invocation token.
- `openEphemeralWindow(url, token)` creates a fresh token when none is given.
- A matching live token reuses the existing off-the-record
  `WebEngineProfile`; normal and private windows do not enter this reuse path.
- Registration rejects a live token bound to a different profile; final
  unregistration removes the token's owner entry and strong profile reference.
- Secondary views and opener popups bind to the effective shared profile.
- Secondary request-interceptor attachment is skipped for shared profiles so
  the owning primary interceptor is not replaced.

## Verification

Automated static coverage is in
`ephemeral_hint_target_creates_a_typed_new_window_action`. Full QML runtime
qualification of download routing and final-owner destruction remains part of
the V1.1 acceptance test.
