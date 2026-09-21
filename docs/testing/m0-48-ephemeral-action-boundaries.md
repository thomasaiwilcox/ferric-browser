# M0-48 Ephemeral action boundaries

Date: 2026-09-16  
Status: in-progress  
Requirement: EPROFILE-004

## Implemented slice

The reducer privacy model now exposes a shared transient predicate for private
and ephemeral profiles. Engine-facing action boundaries use it consistently:

- page userscripts and explicit userscript execution inherit the transient
  privacy gate;
- switcher tab/window results are excluded by the normal-profile filter, and
  transient switcher activation is refused;
- Site Doctor treats ephemeral documents like private documents;
- saving, loading, previewing, or deleting durable sessions is refused;
- context creation, entry, save, deletion, and durable route management are
  refused without opening durable stores.
- the profile registry can be read through a worker from a transient window;
  `profile-open NAME URL` requires an explicit safe URL there and the native
  library manager offers a preview that transfers only that URL into the
  selected named profile.

Session permission decisions remain memory-only, while site-scoped durable
permission rules can only reach an opened normal-profile store. Safe page
navigation and ordinary in-profile tab actions remain available.

## Verification

The core and Qt-engine tests cover the transient model and boundary wiring;
`cargo xtask check --locked --offline`, strict workspace clippy, and the
offscreen Qt/QML adapter smoke pass. Final ephemeral-owner lifecycle
qualification remains a later slice.
