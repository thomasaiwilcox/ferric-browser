# C01 — Preserve existing ownership; extend only what is missing

## Current foundations

Use the latest code map in BASELINE.md. `FerricHintWorkspace`, `FerricPageSearch`, `EngineRequests.js`, `FerricProfileResourceRegistry`, `FerricProfileResources`, native `FerricProfileLifetime` and application `ProfileSession` are existing foundations. Do not recreate them under new names or flatten them back into a giant QML root.

Keep TabId as browser-page identity. Native window/host identity, actual profile-instance identity and page document identity are different. A captured page operation binds at least the exact UI/runtime owner, TabId, tab generation, DocumentId and view incarnation. Per-window Rust runtimes must not be assumed to share an ID namespace without checking; use existing global owner tokens where needed. A tab index or page title is never sufficient authority.

QML projects state and runs effects. Rust core/application owns policy and transitions. Native Qt objects own platform lifetime and transport. Follow existing captured engine-request execution and current config/storage APIs. Register new bridge invokables in the current `bridge.rs`/build resource mechanism; do not assume all invokables still live in `lib.rs`.

## Shared lifetimes

One live native profile/partition can have several page hosts. Sharing a profile does not authorize sharing private UI results, hint selection or credential requests. Reuse shared configuration/permission/download owners rather than reopening or independently owning durable resources for each window. Distinguish a logical named context from a real storage/isolation partition when forming keys.

The application service lifetime must outlast surviving real pages. Closing the first visible window must not destroy the shared root, profile, helper owner or other windows' controllers. Prefer the current application-lived service container or a narrow extraction; a hidden zero-page service object is not a daemon and must not keep the app alive after ordinary shutdown conditions are met. A spare is not a real user consumer and cannot prolong a private session or last-window lifetime.

Maintain separate notions of selected page, keyboard focus, mapping, visible/occluded state, renderer lifecycle and active prompt. Several tiled pages may be visible simultaneously. Unknown visibility never authorizes freezing/discard. Existing media, capture, downloads, dirty forms and pending-auth safeguards remain effective.

## Integration constraints

Preserve native popup `acceptAsNewWindow`/request handoff, opener communication, form-reload decisions, native chooser ownership, close/undo semantics, clipboard, start-page drafts and current search direction/selection behaviour. Existing private Qt download guard remains qualified; do not modify it to ease a new build. New private API use requires a separate explicit architecture decision, not a worker workaround.
