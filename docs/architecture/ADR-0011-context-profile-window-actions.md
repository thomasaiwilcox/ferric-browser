# ADR-0011: Context, profile, window, and shared action subjects

- Status: accepted for the initial V1 implementation
- Date: 2026-09-18
- Scope: context identity, profile affinity, window/tab subjects, and action
  dispatch

## Context

RustBrowser needs contexts, profiles, windows, tabs, and stored browser
objects to remain distinguishable across asynchronous UI, IPC, switcher, hint,
and userscript paths. A label or current selection is not a sufficient target:
it can become stale while an operation is queued, and a context must not
silently become a second profile boundary.

The same operation also needs to be discoverable from commands, typed IPC,
the universal switcher, hints, and browser-owned menus without maintaining
separate action implementations.

## Decision

1. A context is presentation/task metadata above exactly one normal profile;
   it does not own cookies, a Rust metadata store, or an authentication
   boundary. Private and ephemeral windows do not receive durable context
   membership.
2. Runtime subjects carry stable typed IDs plus the captured window/profile,
   tab generation, and document generation where applicable. Every execution
   path revalidates those facts at the final effect boundary.
3. The shared Rust action registry is the source of truth for namespaced
   subject/verb actions, typed arguments, effect class, confirmation policy,
   sources, and capability requirements. UI, IPC, switcher, hint, and
   userscript actions map back to the existing typed command path.
4. Cross-profile tab/view movement is rejected. Context routing is
   prompt-oriented and validates context/profile affinity before mutating
   window membership.

## Consequences

The universal switcher and browser-owned surfaces can focus exact live targets
without trusting a stale title or index. Stored targets must revalidate safe
URLs and privacy/capability state before navigation or execution. Compositor
activation and native accessibility still require platform qualification, but
they cannot weaken the identity checks.

## Evidence

- `crates/browser-core/src/action.rs`
- `crates/browser-core/src/model.rs`
- `crates/browser-core/src/reducer.rs`
- `crates/browser-engine-qt/src/lib.rs`
- `docs/testing/m0-170-switcher-action-activation.md`
- `docs/testing/m0-44-context-visibility-workspace.md`
- `docs/testing/m0-69-action-audit-errors.md`
- `docs/testing/m0-177-profile-bootstrap-worker.md`

## Reconsideration trigger

Revisit this decision only if a required platform capability proves that
profile affinity cannot be preserved, or if an explicit product decision adds
cross-profile data movement. Such a change requires a new ADR and migration
tests; it must not be introduced as an adapter convenience.
