# Binding implementation contracts

This index freezes the required Section 3 boundaries for implementation and
review. The authoritative detail is Section 3 of
`../../../Ferric_Site_Policy_Advanced_Hints_Implementation_Plan.md` relative
to this document. Section 4 supplies the integration map. Unimplemented
requirements below remain open; existing tests establish only the stated
baseline or static subset. Acceptance IDs are mapped in
`site-policy-acceptance.json`.

Configuration compatibility: the optional `SiteRule.interaction` field keeps
top-level schema version 3 and the separate existing generated-runtime schema
version. It requires a build containing the typed `InteractionPolicyPatch`
parser, strict field/null validation and static Interaction capability metadata.
The pinned pre-feature revision `f2c5d6deebd97eb861084503f25613a6a819949d`
does not have that parser and rejects the unfamiliar field. The ordered P02
exit source/build fingerprints identify the supporting development snapshot;
no released minimum version or complete runtime capability is advertised yet.
Static remap capability entries serialize as `required`, never as qualified
delivery. P01's narrow native experiment does not grant P10/P16 qualification.
Release-facing compatibility documentation remains P17 work.

Binding identity uses the shared canonical token vector in parsing, layer
replacement, resolver construction, query/explain, help and guide conflicts.
Display retains authored catalogue keys; metadata indexes retain source keys
such as `+`, `g,g` and `Ctrl+v,g` without concatenating and reparsing them.
Active user-source classification comes from the effective configuration;
layer inputs supply contribution metadata only. The focused production CTest
entries are `application_binding_aliases` (Qt help/guide and input delivery)
and `application_binding_alias_ipc` (public CLI queries against a disposable
production browser). These entries do not qualify the later native remap matrix.

| Section | Observable invariant | Owner and interface boundary | Evidence / owning phase |
| --- | --- | --- | --- |
| 3.1 | One URL matcher and policy publication authority; immutable effective snapshots; Hint session and physical gesture capture generations; no parsing/DOM/focus roundtrip in physical handler | Config routing, application configuration publication, core typed Interaction consumer, BrowserUi snapshot, existing native router | Source catalogue/startup transport and immutable value snapshot tests pass; P03–P04/P09–P10 publication/routing pending |
| 3.2 | Absent leaves do nothing; lists replace or clear; empty tables no-op; enabled=false retains data; receiver descriptor replaces atomically. Separate ordinary/enhanced label accumulators honor explicit higher shared leaves; applicability decided before discovery | `InteractionPolicyPatch` and sparse configuration sources feed one resolver; typed ordinary/enhanced labels feed Hint session | Schema/legacy/source tests and ordered value composition/applicability tests pass; source-aware resolver and usable codebook integration P03–P04 pending |
| 3.3 | Higher source beats priority; priority/authored order within source; same ID shadows whole lower rule before URL match; distinct IDs remain distinct | Existing `routing.rs` matcher and generated runtime site-rule IDs, shared resolver | Source inventory records corrections; P03 precedence and exact-ID editing pending |
| 3.4 | Validate all sources, including hidden/disabled/shadowed input, and composed effective values; reserve/write/publish atomically; failed edits preserve revision; truthful saved/active receipts | Config static compiler → application reservation/write/publication → BrowserUi receipts | Static selector/key/schema/source validation subset; effective validation and publication transactions P03/P11/P13 pending |
| 3.5 | Authored invocation families determine applicability; bounded conjunctive predicates use exact documented normalization; selector admission stays existing discovery configuration | Config predicate/root compiler → bounded isolated-world facts → browser-owned candidate annotations | Static predicate/selector and compiled name tests; renderer evaluation and proof rejection P05/P14 pending |
| 3.6 | Explicit typed roots and per-item paths; nested traversal shares depth/work bounds; unique scope intersects candidates; browser validates rooted identity/generation before authorizing actions; foreign frames require canonical permitted origin and captured broker authority | PageInteractionScripts and existing frame provider supply bounded facts; core/browser validate typed proofs | P00 fixture inventory covers subjects and negative boundaries; P05 broker/identity/security implementation pending |
| 3.7 | Collection order is authored rank with holes retained; ambiguity cannot choose an arbitrary winner; per-item targets are bounded and remain distinct even with identical hrefs | Renderer collection facts → core ordered candidate/reservation compiler | Grid/sidebar/duplicate/recycling baseline fixtures; P06 matching/ranking/witness implementation pending |
| 3.8 | One codebook is prefix-free; special reservations are monotonic for session lifetime; consume/refresh never reuse retired ownership; explicit no-selection remains distinguishable; all families retain action semantics | Core `hints.rs` owns codebook/HintInteraction; workspace presents typed selection and action permits | Existing ordinary Hint interaction/refresh tests; enhanced codebook oracle/reservations/no-selection P04/P06–P08 pending |
| 3.9 | One shared canonical token parser; reserved rescue cannot be source/output. Hint sources excluded at every label position. Committed page commands clear only label-selection state before submission and return the same Hint invocation to Normal after a successfully forwarded gesture's final release; blocked/cancelled commands preserve Hint mode. Early slash makes following letters query text | Core key token and interaction reducer → BrowserUi routing decision → existing native adapter; no queued native events | Static key canonicalization and existing early abstract-input queue tests; enhanced routing/provisional query P08–P10 pending; user-approved one-action Hint completion added 16 November 2026 |
| 3.10 | Frozen physical-source ledger and receiver-scoped normalized-output ledger; first committed transformed collision wins, losing source stays suppressed through release; unchanged-native pairs retain baseline behavior. Original key metadata stays intact for unchanged/raw; virtual destinations cannot copy incompatible scan metadata | Native router owns physical/output state; BrowserUi supplies frozen typed route and observed editable/noneditable/unknown focus with document/frame generation | Native baseline and ASan lifetime/reentrancy tests; new ledgers/virtual metadata/real input/receiver-safe cancellation P01/P09–P10/P16 pending |
| 3.11 | Raw bypasses transformations within existing passthrough session; held transformed/cancelled sources refuse raw entry. Protected capture retires only stale cancelled source on fresh physical down/up; retry raw explicitly. One-shot ends on substantive real source release, retaining overlapping ownership | Typed mode command/parser/IPC → core session state → native source ledger/recovery → presentation | Existing passthrough, overlap, Ctrl+Escape and primary Ctrl+Shift+Escape production regressions pass; new raw/refusal/capture P09–P10 pending |
| 3.12 | Named product bounds apply at authored/effective/renderer/queue/draft/import boundaries; shared work counters span rules and nested roots; byte cap does not claim execution-time bound | Config constants/validators, core allocation, shared renderer pass counters, draft/import coordinator | Static limits subset; effective/renderer/transaction boundary cases P02–P15 pending |

Browser-owned code grants actions; page-authored facts never grant browser
capability. Unknown focus is unsafe for new commands. Cleanup never sends a
release to a replacement document/frame/receiver. Public native delivery cannot
atomically lock DOM focus: tests and documentation must expose that boundary.

The P02 name compiler in config `interaction_name.rs` applies NFC, trims and
collapses Unicode White_Space to ASCII space, then optionally folds ASCII case.
Both operands follow the same path; attribute values and URLs do not use it.
The decoded authored operand is bounded to256UTF-8bytes before normalization;
canonical expansion is retained intact. Complete name evidence has the existing
hint decoder's512character/2048byte input bounds. Missing/truncated evidence is
unavailable, including for empty operands. These types assert text quality only:
being within a size bound cannot prove extraction completeness. Existing renderer
text silently truncates and must not be passed as complete without new proof.
Root/candidate association, renderer completeness and actual matching remain
P05/P06 integration work; this static compiler does not authorize an action.

Existing no-policy ownership and its executable coverage are indexed in
`site-policy-no-policy-ownership.md`. Fixture subjects and their observed
outcomes are in `site-policy-fixture-inventory.md`. Neither index closes the
native transformation/platform or enhanced-policy acceptance requirements.

`InteractionPolicyBuilder` composes preselected rules in caller-supplied order;
it never matches URLs or replaces the existing publication coordinator. Duplicate
IDs reject because whole-rule shadowing must happen before composition. Profile
and CLI sources supply shared settings only. Snapshots retain bounded winning
field origins and typed profile/revision identities. Full shadow explanations
use the captured resolver catalogue in P03. A finished snapshot contains resolved
configured values and validated syntax, not a usable codebook or native permit.
Effective validation shares the authored checks with separate 32 KiB selector /
128 KiB serialized caps; the authored 16 KiB /64 KiB caps remain unchanged.

Static remap compilation exposes immutable exact source/destination/context and
receiver-scope capability requirements. The serialized qualification is always
`required`; receiver origins stay in the effective policy for separate admission.
Class membership does not qualify another key, backend, layout or modifier chord.
The inventory is bounded by32remaps ×4contexts. Static overlap/control/output
checks and reciprocal one-hop lookups pass; live admission, physical adapters,
output ownership and native qualification remain P01/P09/P10/P16 work.

P02-05 now has immutable compiled conjunctive predicates over borrowed complete
facts. Names alone normalize; attribute values/encoded href paths stay exact.
Role tokenization uses five HTML ASCII whitespace bytes. Complete absence,
empty presence, missing and truncated evidence are distinct; unknown facts
cannot prove a match. Shared100000step work persists across all rules/items
and charges failed/unavailable operations and actual glob state transitions.
The existing8192byte href cap is retained; no new observed attribute/role cap.
Renderer extraction/completeness, candidate/root association, projected primary
fragment choice and action authorization remain P05/P06 responsibilities.

P02-12 annotation boundary (structural decoder verified): engine
`interaction_proof` owns a separate version1 annotation decoder. It checks the
2MiB UTF-8 annotation byte limit before deserialization; ordinary candidate
JSON and browser-retained candidate snapshots remain outside that limit.
Sequence visitors enforce declared counts during decoding. Externally tagged
enums avoid buffering whole tagged bodies before those visitors execute.
Document strings are all validated before any parent walk uses them, preventing
unchecked large ancestor paths from causing repeated prefix allocation/scans.

Requests capture the immutable effective policy, invocation family, opaque
profile/window/tab/document IDs, all revisions, view/viewport/collection/provider
generations, request ID, scope lease echo and permitted frame-owner catalogue.
Every u64 echo is a canonical decimal string, preserving values above2^53.
Each enabled applicable anchor/collection has exactly one outcome. Unknown,
omitted, duplicate or wrong-category references reject the whole annotation.
Unavailable outcomes contain a closed reason and no membership/action payload.
Complete anchor outcome cardinality supplies absent/live/ambiguous facts; it is
not a renderer-selected label or command.

Document/root identities use nonzero32bit references. DOM object IDs share the
collector's per-document namespace extended from existing Element identities;
Document and ShadowRoot IDs cannot reuse Element IDs. Root paths preserve
ordered host/document/shadow/direct-slot associations. Repeated identical root
associations are legal; conflicting current host/root/document/slot mappings
reject. Known document ancestry, rule-root and per-item paths share depth8.
These are checks on the supplied graph, not proof of actual DOM ancestry.

Candidate witnesses bind exact index, frame owner/path, element and optional
related identity to the existing validated catalogue. Collection items are
separate structural nodes and need not be ordinary candidates. Each primary
root starts at its exact item; duplicate primary ownership cannot silently
produce distinct ranks. Candidate kind, display text, href and meaning are
retained in an owned deduplicated subset, together with the immutable policy.
Serialization omits those browser snapshots; Debug emits counts only.

Raw complete hint_name is independent of legacy display normalization,
truncation and fallback labels. Required missing/truncated evidence cannot
prove a match; an extracted empty name must be complete-present empty text.
Attributes/URLs retain their exact bytes. Target border-box CSS dimensions and
projected primary-fragment/top-viewport CSS geometry are separately named;
finite components and derived edges/centres are validated. Decoder metadata and
predicate operations share one100000step counter; aggregate metadata records
have their own50000limit. Those are decoder counters, not evidence of actual
renderer inspection counts or wall-clock time.

CheckedAnnotations is non-deserializable and grants no action permit. Matching
an old captured request does not establish current session freshness. Owner
whitelisting does not prove focus; a scope echo does not prove containment;
declared epochs/roots do not become broker identities. P05/P06 still must bind
actual retained leases/roots, verify extraction completeness/ancestry/projection
and re-prove live uniqueness/membership before using special bindings.

Evidence qualifications: the80rule/640hop encoding subject has empty matches
and items; it does not establish maximum populated proof capacity in2MiB.
Aggregate-counter unit tests prove boundaries/sharing, not a50000record live
payload qualification. The ordinary5000×8192byte-href test uses the existing
candidate decoder independently of annotation sizing. Live transport, DOM work
and all native/performance gates remain separate outstanding requirements.

Final structural evidence: `target/site-policy-baseline/p02-proof-green.log`
(30 focused release tests); `p02-proof-engine-full.log` (605 passed, one
pre-existing ignored); `p02-proof-clippy-final.log` (strict release lib/tests);
`p02-proof-kde-policy.log`; six verified inputs in
`p02-proof-final-source.sha256`. Ten regression failures against the first
validator are retained in `p02-proof-regressions-red.log`. Independent final
review found no blocking structural defect. These checks close P02-12 only.

P02-11 verified typed contracts; P03 runtime integration remains open: `site_policy_rules` reuses the
existing HTTP(S) matcher, validates raw rules, replaces whole IDs before
matching, and orders selected rules by source then priority/authored index.
The authored source is the last explicit include-merged array, with exact
catalogue equality checked; earlier arrays are never rematched. Standalone
configuration uses `PolicySourceIdentity::InMemory`, granting no file-edit
identity. Replacement chains retain source/revision/location provenance.

`SitePolicySection` is a compile-time typed Patch/Accumulator/Snapshot contract.
The common composition loop validates complete consumer inputs before applying
selected patches and final effective validation. Interaction uses that loop;
the second consumer exists only in application unit tests. Shared-label leaves
still need interleaving at their actual source boundaries in P03; a bulk site
feed must not be mistaken for the full configuration compiler.

`PreparedConfigurationUpdate::prepare_consumer` retains owned compiled output
privately with the exact prepared state. `publish_configuration_consumer`
delegates existing freshness/reservation/persistence/publication unchanged and
returns output only on success. It installs no runtime cache. A compiler sees
the post-boundary effective projection and desired source layers and must
respect pending settings; reload-required desired rules are not automatically
active. No consumer compilation is allowed after persistence.

Initial rule-feed tests pass10cases (`p02-rule-feed-third.log`). Initial typed
consumer tests pass4cases (`p02-typed-consumer-first.log`), but independent
review found incomplete dummy raw-inventory checking and seed checks skipped
for rules without Interaction. Those initial passing tests were insufficient; corrected-source evidence follows.

Final P02-11 evidence: four regression failures are retained in
`p02-typed-review-red.log` and `p02-typed-seed-red.log`. The complete dummy
inventory now compares against a separately enumerated inventory from the exact
prepared source layers. Bulk Interaction compilation accepts only shared-leaf
seeds, preventing old site rules from surviving even nonmatching replacements.
A positive seed case preserves shared leaf values and source provenance.

`p02-typed-consumer-final.log` passes335release tests (158 config;177 application),
including real stale/reservation/persistence rejection, pending reload-only
rules, replaced include arrays, source ordering and unknown future tables.
`p02-typed-clippy-final.log` passes strict all-target release Clippy; format/diff
and current KDE policy pass. All10inputs in `p02-typed-final-source.sha256` verify.
Two independent final reviews approve P02-11 only; P02/P03 gates, runtime cache
installation, full shared/global interleaving and native/performance evidence
remain open. Evidence paths share `target/site-policy-baseline/`.

P02-10 verified partial resource boundary: outer config/runtime/profile/context
reads use actual remaining bytes plus one sentinel, retaining the2MiB aggregate
contract and existing include depth. Exact UTF8 source sizes, aggregate siblings,
zero-byte children at exhaustion and depth/nesting rejection preserve ConfigStore
publication. A counted short-read source proves the helper consumes at most
remaining+1bytes, without relying on file metadata.

Diagnostic rendering has a1024UTF8byte budget and256byte path context. TOML
input is detached before formatting (retaining message, span and available field
path), so full source lines/caret padding are not copied into the stored error.
Control escapes, including Unicode line separators, remain indivisible through
wrapping/truncation, even next to literal backslashes. Full paths remain in
path-bearing variants; validation strings retain only rendered context. This
does not cap accepted configuration text, public caller-supplied error fields,
or all untouched legacy stored-error producers.

Static count tests cover Section3.12 entry bounds, independent UTF8 predicate
operands and cross-source511/512/513reservations with replacement-clear repair.
They do not qualify filtered fallback capacity, DOM work, runtime draft/preview/
capture limits or interchange services. Those and the complete P02-10 audit
remain open. Final evidence: p02-limits-final.log356tests (179config;177application),
p02-limits-clippy-final-reviewed.log strict all-target release, formatting/diff,
p02-limits-kde-policy.log and nine verified p02-limits-final-source.sha256 inputs.
Two source reviews found no remaining defect in this reviewed subscope.

P02-13 verified: the optional engine annotation fuzz driver uses the production
decoder and fixed independently captured request/candidate inputs. It requires
the exact referenced snapshot keyset/meaning and retained policy, handles raw
versus canonical byte-size differences, and has an injected missing-snapshot
test proving its oracle detects loss. Normal builds omit the driver; the
default13-target fuzz workflow retains its Qt-free95-package active graph.
The optional feature uses148exact existing root production identities/checksums
plus3unchanged fuzz-only identities. No copied decoder or second publisher.

Seven new engine property/transaction tests cover successful anchor/collection/
focused-owner/shadow-slot/unavailable seeds, incomplete JSON prefixes, metadata
and geometry mutation, budget exhaustion, actual prepared publication and
same-owner recovery after an injected compilation panic. No real parser panic
is claimed. The legacy reload-worker assertion now verifies the actual bounded
Check diagnostic/path/byte span and competing-request ownership, replacing its
obsolete expectation of a raw configuration excerpt. Production worker code
is unchanged. Full engine suite:612passed, one pre-existing opt-in performance
test ignored (`p02-proof-fuzz-engine-reviewed.log`).

Default smoke13×32passes (`p02-proof-fuzz-default-smoke.log`); optional SDK
smoke12seeds/128runs passes (`p02-proof-fuzz-optional-smoke.log`). Both are
uninstrumented corpus smoke. Strict engine/xtask Clippy, formats, diff and
current KDE policy pass. All23inputs in p02-proof-fuzz-reviewed-source.sha256
verify after execution. Two independent final reviews approve P02-13 only.
Coverage-guided campaigns, all policy/root combinations, live DOM/broker and
native/performance qualification remain distinct outstanding work; P02-10 and
the full phase gate remain unchecked. Evidence shares target/site-policy-baseline.

P02-10 final-scope audit complete: shared product-limit definitions now also
cover inspections/depth, abstract early intents and zero native-event backlog,
draft/preview/capture cardinalities and local interchange. Independent numeric
contract assertions live in application `tests/site_policy_bound_contracts.rs`.
They freeze definitions rather than demonstrate future runtime enforcement:

| Consumer | Remaining implementation and qualification |
| --- | --- |
| P04 | Fallback alphabet after exclusions, enhanced codebook capacity and eight-character codes |
| P05 | One shared DOM inspection/depth/predicate budget across collector roots and ordinary discovery |
| P08 | Bounded early abstract intents, with no queued native events |
| P11/P12 | Draft/preview/capture lifecycle, bounded immutable receipts and no unbounded history |
| P13 | Bounded reviewed interchange and one atomic import batch |

Existing ordinary allocator and structural proof limits consume the shared
constants where applicable. QML collector/input literals and later services are
not claimed to consume these definitions yet. Existing ordinary computed labels
may exceed eight characters; the enhanced bound must not reduce legacy capacity.

Historical static checkpoint (before the execution-order correction): the
approval and test counts in the following paragraph are retained evidence, not
the current ordered P02 exit verdict. P02 revalidation and its outstanding
checks are recorded in the implementation plan.

The final producer audit also fixes malformed non-array `include` values that
the loader previously removed before typed deserialization, and bounds include
path context so the validation reason survives long real paths. Invalid includes
now reject and preserve the prior ConfigStore revision/snapshot. Valid include
array replacement and eight-level semantics remain unchanged. Raw externally
constructed public error fields retain their data; Display and Debug renderings
are bounded. Final release checks passed:546 core/config/application tests plus
two explicit numeric contract tests;612 engine tests with one pre-existing opt-in
performance test ignored; strict all-target Clippy, formatting, diff and current
KDE policy. All654 frozen source inputs verify. Two independent final reviewers
approve P02-10 and the static P02 gate. Detailed logs and compatibility corrections
are recorded in the plan. Later enforcement and live/native gates remain open.

P03 source-aware Interaction compilation is now verified through
`ConfigurationPolicySources::compile_interaction` and the existing prepared
consumer publisher. Both desired and reload inputs validate; only reload rules
are selected. Sparse desired base/profile labels precede authored sites, while
higher generated/temporary globals follow their own site patches and CLI keeps
its intervening precedence. Global and site origins retain their distinct source
epochs. Authored Hint reconstruction uses existing include leaf merging and
rejects inconsistent catalogues. Unmatched resolved entries stay private.

Final corrected evidence:408config/application tests, strict all-target Clippy,
formatting, diff, current KDE policy and124frozen source/fixture fingerprints
pass; two independent reviews approve P03-01/02/03. The13compiler cases cover all source
boundaries, sparse explicit defaults, whole-ID nonmatching replacement, equal
priority order, include origins, pending/live mixes, both catalogue failure
directions and final513-to-512reservation repair. A post-review regression fixed
standalone resolved selectors being recast as raw authored Hint inputs: shared
label projection now returns only its three relevant leaves, while full base/
catalogue validation retains the existing resolved32/authored16selector limits.
The16/17/31/32acceptance and33rejection case passes. Logs use
`target/site-policy-baseline/p03-interaction-compiler-*`. This qualifies pure
source compilation. Runtime setting application, selector/proxy correction,
caching, engine/native evidence and the full P03 gate remain open. Profile
identity/revisions supplied to the pure compiler remain browser-owned context;
the runtime cache must bind them to its actual owner. Publication-owner and
context-publication findings from the next P03-10 audit remain open in the plan.
