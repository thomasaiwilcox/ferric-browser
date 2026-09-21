# M0-45 Context save descriptors

Date: 2026-09-16  
Status: in-progress  
Requirement: CTX-003

## Implemented slice

Context membership snapshots now keep bounded restore descriptors alongside
the current runtime window/tab IDs. Each descriptor contains a stable
runtime-tab reference, a safe restore URL, sanitized title, pinned/muted
state, and bounded zoom. The selected tab is recorded by descriptor index.
Existing state files without descriptors remain readable.

`context-enter` now focuses a matching live member tab when its runtime
identity is still valid. Otherwise it materializes the most recent saved
descriptor set with fresh runtime tab IDs. The selected descriptor is
navigated immediately; other restored tabs are represented in the tab model
but remain unloaded until selected. This keeps post-restart restoration
lazy and avoids replaying unsafe or stale runtime identities.

Entering a context whose default target requires a separate window now emits
an explicit typed context-window action with the context profile and selected
safe restore URL. QML creates that native window, marks the startup as an
intentional restore, and forces the startup context-enter to reuse the new
window rather than recursively creating another one. Existing registered
context windows are focused first, including their workspace request.

context-save records only live tabs with URLs accepted by the existing
profile-local durable-history policy. Private profiles cannot save context
state, and unsafe or transient URL descriptors are omitted with a count in
the structured result. Saving remains an atomic generated-state update and
does not alter the user-maintained contexts.toml document.

The secondary native-window surface still exposes one web view; full
multi-tab/multi-window restore qualification and compositor end-to-end
qualification remain.

Generated context membership writes now use the same fault-injection seam as
session snapshots. Failures at temporary-file sync, replacement rename, or
directory sync leave the in-memory record rolled back; reopening proves the
on-disk registry remains valid and exposes either the retained old membership
or the fully renamed new membership, never a partial JSON file.

Model hardening: generated `ContextRecord` and `ContextMember` state now uses
strict serde schemas, so unknown fields cannot silently become part of the
durable context model. The existing bounded validation still enforces unique
UUID/slug identity, one profile reference, ordered session references,
workspace/accent/default-target metadata, timestamps, and safe generated
membership descriptors.

When generated membership exists, changing the context's profile affinity is
refused with an explicit migration error; the registry never silently moves
saved windows or engine storage across profiles.

## Verification

Commands: `cargo fmt --all -- --check`; `cargo test -p browser-engine-qt -p
browser-storage --locked`; `cargo xtask check`; and
`QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`.
