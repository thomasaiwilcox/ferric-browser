# ADR-0001: Keep the application reducer independent from Qt

Status: accepted for the initial implementation slice  
Date: 2026-09-15

## Decision

Ferric Browser starts with a Qt-independent `ferric-browser-core` crate. Application
state is changed only by typed events through a reducer, which returns typed
engine, persistence, and diagnostic effects. GUI and desktop adapters will
consume this boundary; they do not own browser policy or identity.

## Rationale

This directly implements ARCH-005 and STATE-002/004: core tests can run without
a display server, Qt objects remain on the GUI thread, and stale callbacks can
be rejected using captured tab/document generations before a Qt adapter is
introduced. It also allows the engine feasibility work to be tested separately
from browser policy.

## Consequences

Runtime IDs are monotonic opaque IDs for one process. Durable UUID-backed IDs
and persistence schemas belong to the storage milestone. URL validation is
currently a small core boundary for supported browser schemes; search/query
resolution is intentionally deferred to M1 navigation work.

