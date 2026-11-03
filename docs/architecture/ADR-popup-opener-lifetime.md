# ADR: Native lifetime leases for popup openers

**Status:** Implemented candidate; component qualification pending
**Date:** 2026-10-25

## Context

Qt 6.11.2 WebEngineQuick can leave a nested popup's native widget delegate
holding a pointer to its opener `WebEngineView` after the opener's window is
closed. Qt's public `QObject` metadata, `QQuickItem::childrenChanged`, and
`QObject::destroyed` signals let Ferric observe the lifetime of the known
`QtWebEngineCore::RenderWidgetHostViewQtDelegateItem`. The class name is a
private implementation detail even though the observation APIs are public.

## Decision

`FerricProfileLifetime` owns one directed, opaque-ID lease for each adopted
popup edge. Admission requires distinct tracked current views with the same
actual profile object, the same `QQmlEngine`, ready registered hosts, and the
GUI thread. The lease is acquired before `NewWindowRequest::openIn`.
The caller marks the lease started immediately before `openIn` and confirms it
immediately after that call returns. A bootstrap that fails before the start
mark cancels the lease; cancellation is rejected after the start mark. If an
endpoint dies before the start mark, the lifetime cancels the pending lease.
If a started target dies before any delegate appeared, the lifetime waits one
event-loop turn after the target QObject is destroyed, then releases that edge
only if no matching delegate was observed and no delegate watch remains. Once a
matching delegate was observed, actual destruction of every watched delegate
is still required.

The observer matches the exact Qt 6.11.2 metadata name and `QObject::inherits`
identity. This is an SDK-qualified compatibility dependency, not a
version-independent or class-agnostic contract. A different Qt version does
not compile the observer; an unknown runtime name is never treated as a match
or as authority about origin, document, or credentials.

When a leased opener retires, Ferric first emits the existing retirement
signal, then hides and disables the view, sets public lifecycle state to
`Discarded`, detaches its visual parent, and retains its QObject under the
existing profile lifetime. Each edge observes the target's public visual-child
tree for delayed or replacement delegate items and tracks each matched
QObject until its actual destruction. A retired opener is deleted only after
all its outgoing edges have observed at least one matching delegate and those
delegates have been destroyed, with the opener confirmed `Discarded` and
renderer-free. Unknown or never-observed topology fails closed and may retain
the shell while the target remains live; it never triggers a PID guess, reload,
or input workaround. If an already-started target is destroyed before any
delegate was observed, one queued event-loop turn settles the lease only after
target QObject destruction has completed and no delegate watch remains.
Pending cancellation applies only before content binding starts. The separate
post-destruction path applies only after the target WebEngineView QObject and
its WebContents owner are gone; it does not release an edge while an adopted
target remains alive with an unseen or unknown surface.

Per-view and per-edge generations reject stale notifications. The profile
factory and resources remain owned until all views drain. Shutdown deletes
targets before openers using adoption order, then lets QObject ownership
destroy the profile resources.

## Consequences

The native class-name assumption must be rechecked for every supported Qt
SDK. This ADR supports only the exact 6.11.2 build guard. A missing or changed
metadata name leaves popup lifetime support unqualified rather than claiming
safe early release. This mechanism does not change popup navigation or input,
and provides no credential authority or origin/document bridge.

The `everyday_T014c_popup_delegate_lifetime` component test uses the
production C++ lifetime source and real WebEngineQuick views. Its result is
component evidence only; composed-application, compositor, physical input,
security, and release qualification remain separate.
