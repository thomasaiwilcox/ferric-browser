# M0-95: Ephemeral owner accounting

Date: 2026-09-16  
Status: in-progress  
Requirement: EPROFILE-003

## Implemented slice

The root QML window now maintains an explicit owner table for shared
ephemeral WebEngine profiles. Registration increments the count for the
invocation token and rejects a live token that is bound to a different profile.
Unregistration decrements the count and removes the token entry at the final
owner, releasing the registry's strong reference to the profile. Profile
lookup uses this owner table before falling back to the live window entries.

The per-window close path still performs download, capture, prompt, and core
shutdown checks before unregistration. The Rust transient-resource boundary
then clears the corresponding per-window core state.

## Verification

`cargo test -p browser-engine-qt --locked --offline` and `cargo xtask check
--locked` pass. Native multi-window lifetime and download-routing qualification
remain release acceptance evidence.
