# ADR-0015: Versioned Browsing capture, branch and cookie boundaries

- Status: accepted for implementation; native capabilities remain unqualified
- Date: 2026-09-27
- Scope: explicit normal-profile captures, live retention, isolated branches, transient branch export and cookie capability reporting

## Context and authority

ADR-0006 limits ordinary recovery to safe descriptors and keeps private and ephemeral state out of durable storage. ADR-0011 forbids cross-profile live tab movement. ADR-0012 requires final-owner teardown of transient profiles. ADR-0014 keeps the selected explicit scroll target document-local. Versioned Browsing needs deliberate persistence and separate live ownership without silently changing those rules.

The project owner's instruction to implement the supplied Versioned Browsing handoff is the authorisation to establish this additive normal-profile architecture. It does not authorise a private-profile persistence exception, unsupported Qt behavior or a production capability claim. This ADR records the implementation decision; native and release qualification still follow the slice ledger.

## Decision 1: Explicit capture and normal-profile storage

1. A versioned commit is an explicitly requested immutable capture in a separate versioning namespace beneath a validated normal-profile owner. Its manifest and objects are separate from schema-2 `SessionSnapshot` recovery generations. Routine recovery scheduling remains safe-descriptor-only and never requests page or cookie capture.
2. Structural input comes from the admitted reducer revision and validated profile/window/tab/document identities. Delayed page results retain the captured owner and document generation. The application coordinates operations, the storage worker owns disk I/O, and Qt translates native callbacks; only reducer events change browser domain state.
3. An explicit page archive is potentially credential-bearing even when editable fields are omitted. Page and editable capture require their own consent and limits. Storage uses restricted local files and validated containment; no claim of encryption at rest is made. Cookie values and opaque Chromium profile files are never versioning objects.
4. Stored descriptors and archives are data, not authority. Restore validates safe navigation, does not replay POST or forms, and cannot grant permissions, choose commands or execute captured script. Saved scroll numbers do not restore ADR-0014's weak-reference target identity.

## Decision 2: Retention and branch profile ownership

1. A stash retains the same live page instance under an explicit lease. Frozen is an observed lifecycle state, not an immutable memory snapshot. A retained view keeps its original engine profile and captured application owner. View destruction precedes final profile destruction; loss and only-live-copy disposal are explicit.
2. A child branch uses a fresh off-the-record `QQuickWebEngineProfile` and its own compatible native window. It receives new runtime tab identities and may reconstruct safe structural/page state. It never reparents a live parent view across profiles, inherits grants or uses the parent profile as a fallback. A branch is not an OS or remote-action sandbox.
3. Normal-profile lineage metadata may record the child creation. Child history, journals, ordinary checkpoints and cookie changes remain transient. A switch focuses the exact branch owner and does not roll back the parent's current cookies.
4. Explicit export is limited to a branch linked to a known normal owner. A native preview identifies destination and sensitivity before an explicit `commit` exports validated structural and optional page data to that owner. It exports no cookie values, profile database, opaque engine state or ordinary private-window data. Restart yields a durable capture, not a restored authenticated child profile. A generic private or ephemeral window has no export path through this feature.

## Decision 3: Cookie capability and merge authority

1. The public Qt cookie observation surface may produce a diagnostic, potentially lossy mirror. It has no documented enumeration-complete snapshot boundary, batch transaction or Chromium partition identity. An observation revision, quiet period, repeated enumeration or sentinel cookie cannot prove a complete, atomic source snapshot.
2. Capability reporting separates isolated primitive probes from production enablement. A snapshot-seeded branch requires qualified identity, completeness, source boundary, destination seed and read-back for the declared engine version and scope. If any are absent, the request fails with a reason; `--clean` remains an explicit separate branch mode.
3. A three-way cookie planner may operate on qualified synthetic descriptors without applying them. Production merge writes require supported identity and concurrency controls, captured native consent, revalidation and recoverable partial/unknown outcomes. No whole-jar rollback, blind copy, remote action reversal or silent page reload is promised. Unsupported live-profile application remains disabled.
4. No private Qt/Chromium API, live profile database read, renderer checkpoint or remote-debugging endpoint is an implicit workaround. A new backend requires its own reviewed architecture and security decision.

## Invariant review

These decisions retain VB-I01–I18: reducer ownership and captured targets (I01–I02), profile affinity (I03, I16), descriptor-only recovery and private exclusion (I04–I05), honest capability/fidelity and public APIs (I06–I08, I10, I13, I17), durable publication (I09), explicit retention loss (I11), secret redaction and untrusted page data (I12, I14–I15), and terminal cancellation/unknown outcomes (I18). No exception to those invariants is accepted by this ADR.

## Consequences and reconsideration

Structural Versioned Browsing can proceed independently of cookie support. Clean branches can proceed independently of snapshot-seeded branches. Native stash, archive and cookie behavior must be qualified on the declared Qt/Wayland platform before user-facing claims or production enablement.

Revisit this decision only through a new ADR and tests if an implementation needs automatic transient durability, cross-profile live reparenting, a different archive trust boundary, or a cookie backend with different guarantees.

## Evidence

- `CurrentImplementation/ferric-versioned-browsing/PROPOSAL.md`, Sections 2–4, 16, 20 and 23–25
- `docs/testing/versioned-browsing/VB-000.md`
- `docs/testing/versioned-browsing/VB-001.md`
- ADR-0006, ADR-0011, ADR-0012 and ADR-0014
