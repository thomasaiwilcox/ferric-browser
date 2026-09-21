# M0-94: Transient resource release

Date: 2026-09-16  
Status: in-progress  
Requirements: PROFILE-003, PROFILE-004, EPROFILE-003

## Implemented slice

Transient primary and secondary windows now have an explicit Rust lifecycle
boundary after the existing close/shutdown acknowledgement. The boundary:

- cancels pending selection, action-target, userscript, download, navigation,
  and editor requests;
- terminates browser-owned editor work and signals userscript cancellation;
- clears the core state, tab/window indices, journey mappings, session grants,
  operation records, macros, and closed-tab descriptors;
- stops configuration watchers and releases contexts, stores, profile locks,
  storage roots, and session paths; and
- clears sensitive current-page display state before the QObject is destroyed.

Durable profiles refuse this release operation and remain owned for the normal
application lifetime. Qt WebEngine profile ownership is still governed by the
window/shared-profile object graph; final-owner runtime qualification and
download routing remain acceptance work.

## Verification

`cargo test -p ferric-browser-engine-qt --locked --offline` passes, including static
coverage that both primary and secondary destruction paths call the release
boundary. The full workspace verifier remains the integration gate.
