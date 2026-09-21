# ADR-0012: Journey retention and ephemeral-profile teardown

- Status: accepted for V1.1 implementation
- Date: 2026-09-18
- Scope: navigation journey graph, durable retention, private memory state,
  and ephemeral-profile lifetime

## Context

The journey graph is useful for recovery only when it preserves safe
relationships without becoming a second browsing-history export. Ephemeral
profiles need the same distinction: they may support ordinary navigation and
bounded in-memory recovery while guaranteeing that browser-owned durable
state is not created or retained after the final owner closes.

## Decision

1. Journey nodes and edges are profile-local, typed, bounded, and written
   through the storage worker. Nodes retain only safe URL/title/timestamp,
   profile/tab identity, transition, and bounded source metadata. Current-node
   updates are explicit and branch-after-back transitions are represented
   without replaying form submissions.
2. Durable journey retention is bounded to 50,000 nodes and 100,000 edges and
   follows the configured history retention policy. Clearing matching history
   removes incident journey records in the same transaction.
3. Private and ephemeral profiles may retain a bounded memory-only journey
   graph for the live session, but never open the durable metadata store or
   expose private rows through ordinary IPC or default switcher queries.
4. An ephemeral profile has an explicit shared-owner count. Its final-owner
   close path confirms active work, cancels browser-owned requests, destroys
   views before the engine profile, releases transient Rust state, and removes
   only its exact temporary roots. External downloads and clipboard copies are
   user-owned outputs outside this guarantee.

## Consequences

Journey recovery is safe to offer as a V1.1 feature without making private
state durable. Crash/restart behavior remains explicit: durable normal
profiles recover validated safe descriptors, while private and ephemeral
profiles do not participate in durable recovery. Process-kill, filesystem-full,
and long-duration final-owner qualification remain release evidence rather
than being inferred from unit tests.

## Evidence

- `crates/browser-core/src/journey.rs`
- `crates/browser-storage/src/lib.rs`
- `crates/browser-storage/src/sessions.rs`
- `crates/browser-storage/src/lifecycle.rs`
- `crates/browser-engine-qt/src/lib.rs`
- `docs/testing/m0-53-journey-storage.md`
- `docs/testing/m0-56-journey-retention.md`
- `docs/testing/m0-57-journey-private-memory.md`
- `docs/testing/m0-115-journey-worker.md`
- `docs/testing/m0-48-ephemeral-action-boundaries.md`

## Reconsideration trigger

Revisit this decision if V1.1 requires cross-profile journey joins, durable
private recovery, or compositor-owned state to be serialized. Those changes
would alter the privacy and ownership boundary and require explicit scope and
migration decisions.
