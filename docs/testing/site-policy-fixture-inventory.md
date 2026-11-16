# P00 fixture subjects

These subjects establish reproducible inputs. They do not qualify enhanced
Hint policies, foreign-frame broker authorization, remapping or compositor
input. Paths below are relative to `crates/ferric-browser-engine-qt/tests/`.

| Required subject | Fixture | Executable baseline coverage |
| --- | --- | --- |
| Navigation anchors and duplicate names | `fixtures/site-policy/interaction.html` | `SitePolicyBaseline::test_navigation_scope_duplicate_and_nonactivation` |
| Distinct same-URL cards and tall primary | `fixtures/site-policy/interaction.html` | `test_distinct_cards_same_url_and_tall_primary` |
| Grid and sidebar collections | `fixtures/site-policy/sidebar.html` | `test_distinct_grid_and_sidebar_collections`; separate visible layout and identities despite equal href |
| Occlusion | `fixtures/site-policy/roots.html` | `test_root_subjects_slot_frame_and_occlusion`; covered primary excluded |
| DOM reuse / virtualization | `fixtures/site-policy/interaction.html` | `test_recycled_card_invalidates_retained_target_without_activation`; same Element, changed identity/href |
| Delayed insertion | `fixtures/site-policy/roots.html` | `test_delayed_target_requires_a_fresh_collection` |
| Delayed asynchronous collection completion | Application harness local page | Existing `ApplicationTests::hintEarlyInput` primary/secondary `pending-collection` rows; independently reviewed subject coverage; prior passes retained in `application-focused-detail.log`, current source-bound application rerun pending |
| Open shadow and per-item shadow | `fixtures/site-policy/interaction.html`, `roots.html` | `test_shadow_focus_trace_and_ordinary_discovery`, `test_root_subjects_slot_frame_and_occlusion` |
| Direct assigned slots, duplicate assignment, reassignment | `fixtures/site-policy/roots.html` | `test_root_subjects_slot_frame_and_occlusion` |
| Same-origin nested frame chain | `fixtures/site-policy/frames.html` | `test_frame_chain_and_foreign_origin_negative_boundary` |
| Focused opaque-origin negative subject | `fixtures/site-policy/frames.html` | Same test: parent DOM denial and foreign focus; not an allowed broker origin |
| Focused canonical foreign-origin subject | `fixtures/site-policy/canonical-parent.html`, `canonical-child.html` | `CanonicalFrameSubject::test_focused_canonical_foreign_subject`; top discovery exclusion after child focus and public child-scoped discovery |

`SitePolicyBaseline` is in `qml/tst_site_policy_baseline.qml`. Run it using the
qualified Qt 6.11.2 QML runner with offscreen/software environment. Retained
`target/site-policy-baseline/site-policy-sidebar-fixture.log` records 10 cases
passed. The canonical HTTP subject runs with:

```sh
tools/performance/with_qt611.sh python3 crates/ferric-browser-engine-qt/tests/site-policy-http/run_fixture.py --runner target/site-policy-baseline/qml/qml_tests
```

It binds loopback port 8766, requires the port to be free, and serves the local
site-policy directory. Parent uses 127.0.0.1; child uses localhost, making the
origins distinct without an external network dependency. The existing runner
must already have been built with the qualified SDK. Evidence:
`target/site-policy-baseline/canonical-frame-fixture.log`, 3 cases passed.
Independent reviewer `p00_frame_review` found no confirmed fixture/test defect.
Neither result is evidence of production broker routing.

## P05 bounded name evidence

`site-policy-http/tst_name_evidence.qml` runs the actual shared name extractor
in ApplicationWorld and compares it with ordinary collector display text.
The 23 rows cover exact/excess Unicode limits, reference/label/node caps,
dropped children, inspection exhaustion, editable-value exclusion and unused
fallback sources. The maintained HTTP command above discovers this suite.
`target/site-policy-p05-revalidation/bounded-name-compatibility-first-1/record.json`
records all 258 HTTP/Hint checks passing with exact source postcheck. This
qualifies extraction only; complete witness production and transport remain P05.

## P05 structural items and fragment evidence

`site-policy-http/tst_policy_roots.qml` now checks Element-start per-item shadow,
slot and same-origin frame paths, combined/residual depth, exhaustive raw items
beyond slot_count, shared predicate work, failed-capture rollback and actual
scope-lease intersection. These diagnostic roots do not establish final primary
eligibility, membership, ranking or permits.

`site-policy-http/tst_fragment_evidence.qml` checks the shared presentation/proof
helper in real WebEngine: fragment/work boundaries, occlusion, final scope clip,
vertical-line canonical selection, focus policy, nested and scaled frames.
`tst_name_evidence.qml` additionally proves description evidence uses the original
name walk once. Source-matching final records are
`target/site-policy-p05-revalidation/materializer-item-roots-http-second-1/record.json`
(171 passes) and `materializer-item-roots-legacy-1/record.json` (115 Hint passes).
All failures/skips are zero. The normal `xtask test engine` path now runs the
maintained HTTP command after file-based QML checks. The expanded HTTP batch
timeout is180 seconds; per-operation and DOM work bounds remain unchanged.
Final witness transport remains incomplete.

## P05 raw witness observations

`site-policy-http/tst_witness_observations.qml` covers final-row/related identity,
proxy index changes, deliberate selector exclusions versus cap uncertainty,
post-cap disclosure removal, whole proxy-loop and late facet caps, bounded name
facts, raw item/parent/primary observations, annotation exhaustion and exact UTF-8
escaping boundaries. `witness-observations-http-second-1` records all200 HTTP
checks passing; `witness-observations-legacy-1` records115 Hint checks passing.
The latter source adds only the captured
`fixtures/site-policy/witness-observed-anchor.json`, with provenance linking its
actual AppWorld producer and ordinary candidate outputs. This does not qualify
canonical Rust decoding or shipping/native request transport, which remain P05.

Cached geometry follow-up: witness-observations-http-final-1 passes202 HTTP
checks; witness-cached-geometry-scope-1 passes32 focused checks and
witness-cached-geometry-legacy-1 passes115 Hint checks. The focused/legacy
manifests match exactly. Extra coverage proves a complete small scope survives
an unrelated capped shadow branch without rereading its original single-fragment
facts, while child-frame and actual lease-crop cases still require the full proof.
Rust observed-fact decoding and native transport remain incomplete.


## P05 observed-fact normalization and terminal focus

`interaction_proof/tests/discovery.rs` consumes the actual raw anchor fixture via
the production decoder and checks canonical ordered/Unicode/numeric predicates,
unknown versus definite exclusions, validation-before-filtering, original orphan
rejection, strict/raw shared remaining work, nested included ancestors through
excluded items, self-primary ineligibility and retained graph/snapshot association.
`observed-decoder-second-1` records71 focused proof tests plus strict five-crate
all-target Clippy/static checks passing with exact2304-file source postcheck.
Its independent component review approves normalization only. Original native
request/proof shipping transport and the P05 exit remain incomplete.

`PolicyFrameMapping::test_captured_leaf_requires_no_focused_descendant` reproduces
real deeper-focus acceptance in `focused-terminal-leaf-red-1` (3pass/1fail, no
skips, exact source). The terminal-leaf correction adds malformed/timeout,
stale-document, focus-changing reply and bounded open-shadow focus checks to
`NativeDocumentReceipts`. Full native/HTTP qualification is currently running as
`focused-terminal-leaf-first-1`; no success is inferred before its terminal record.


Terminal-focus follow-up: the full `focused-terminal-leaf-first-1` run has212
passes/1existing5000-row callback timeout; isolated unchanged cap retest3passes
in9.614seconds does not replace aggregate success. The independent closed-shadow
race repro `focused-terminal-closed-red-1` has6passes/1failure. The interim unknown
focus guard now passes79 native/frame/provider checks in
`focused-terminal-leaf-third-1`, zero failures/skips, exact2307-file source.
Potential shadow-capable active terminals with child frames conservatively remain
unavailable; exact child-host association must remove valid-case restrictions
before full P05 qualification. Compile failure second-1 is retained.

Three new raw-observation seeds in `fuzz/corpus/interaction_proof/` extend the
existing production-decoder driver; checked raw outcomes are normalized before
its strict round-trip and candidate/policy retention invariants. New collection
count/original-index/shared-primary regression execution remains pending in
`observed-decoder-full-engine-1`. All P05 checkboxes and its gate remain open.


## P05 exact local Document/native association

`site-policy-http/tst_document_bindings.qml` runs through the shipping native
receipt/broker continuation and collector. It covers unfocused same-origin
Documents without child tokens, exact weak host/parent/current-Document identity,
MainWorld spoofing, batch/stamp/revision replay, actual same-URL child reload and
host replacement, lifecycle retirement, foreign world plus local descendant,
independent2MiB UTF8 document table bounds, absolute GUI-delay expiry, raw subset
scope and native-base+local depth8/9. `native-document-boundaries-red-1` retains the
real retired-batch/global-nine failures (6pass/2fail); both now pass. Earlier native
red also retains two fixture lexical-scope mistakes, corrected with IIFEs and
actual replacement fences.

`native-document-bindings-first-1` passes28focused checks; full maintained HTTP
`native-document-bindings-http-first-1` passes250checks and the six ordinary Hint
suites in `native-document-bindings-legacy-1` pass115checks. Failures/skips arezero;
all source postchecks are exact2309 files and use the same qualified native runner.
Independent review verifies archives/source/runner and approves this limited
association component. It does not prove inventory completeness, full root or
witness freshness, receiver admission, action authority, heap freedom or P05 exit.
The raw document inventory must be sealed before normalization prunes it; adapter,
provider/workspace integration and complete role catalogue remain required.


## P05 original request and focused receiver Documents

The real application fixture `hintRequestNativeOwnership` plus
`hintRequestObservationDocuments` qualifies the opaque original capture,
pre-async browser snapshot, selected-view/native Top URL join, one-shot raw
Document staging, raw excluded-document retention and native association.
`request-observation-documents-engine-2` passes strict Clippy,720 Engine tests
(one existing intentional ignore), format/diff/KDE policy. Its sequential
`request-observation-documents-native-2` shipping/native application run passes26
checks without failures/skips; both records retain exact2310-file source checks.

`PolicyFocusedTopRoots` adds actual native same-origin leaf metadata, canonical
Top IDs, shared depth/predicate/candidate accounting, real child scope leases,
Top/sibling/ancestor substitution and empty focused results. Full HTTP
`focused-receiver-native-first-1` passes274checks, zero failures/skips, exact2310
source files and rebuilt/hashed Qt6.11.2 native runner. This does not exercise the
Rust adapter; `focused-receiver-engine-1` and its subsequent application run are
pending for that frozen source.

New, not-yet-executed role regressions live in
`interaction_proof/tests/receivers.rs`, the existing application fixture and
`PolicyFocusedTopRoots`. They cover structural-only ancestors, native origin and
full depth, same-owner path rules, canonical Top deduplication, foreign and
return-to-Top-origin chains, rejected nonterminal/unfocused admission and focus
moving deeper during the final receipt pass. No runtime result is claimed for
this later delta. Full world-local foreign proof merging, shipping start/refresh,
root/hop/witness/scope liveness and the P05 gate remain incomplete.


## P05 manual slot assignment tracking

`PolicyRootDiscovery` reproduces manual assignment/ABA without DOM mutation,
then checks listener retirement at stop and detached-root pruning. Real minted
scopes cover both an assigned scope root and a nested scope below its assigned
light-DOM ancestor. `PolicyDocumentBindings` checks positive native association,
ABA retirement, fresh-batch admission and permanent old-batch rejection, including
cap1 where a target is admitted before its assigning ShadowRoot is visited.
The tracker reuses composed-ancestor style reads to register exact assigning
slots; no new ancestry/subtree scan is introduced.

Retained reds: `slot-assignment-invalidation-red-1`4pass/3fail and
`slot-assignment-scope-red-1`10pass/1fail. Final focused run
`slot-assignment-nested-scope-first-1`13pass/0fail/0skip; ordinary
`slot-assignment-legacy-first-1`115pass/0fail/0skip. Exact2312-file source checks
and the unchanged qualified runner hash are retained. Independent review approves
the scoped fix, not complete live root/scope sealing or the P05 exit.


## P05 weak root and target object capture

The private collection batch retains exact weak node/owning-root/assigned-slot
objects and finalized target/related pairs plus its original bounded annotation
string. `PolicyWitnessObservations` checks real shadow/slot and proxy objects,
registry retirement, structural scopes/containers, raw excluded items and nested
parent references. Retained red `proof-object-capture-red-2`:2pass/2fail; focused
`proof-object-capture-raw-items-1`:5pass/0fail; affected module
`proof-object-witness-regression-1`:35pass/0fail (known5000callback case excluded).
Exact source/native hashes remain in each record. Native live graph/scope
consumption and P05 exit remain unimplemented/unqualified.

The role snapshot's fullHTTP `receiver-roles-http-1` has275pass/1existing5000
callback timeout. Its new focused admission/late-seal checks pass. Separate
same-deadline phase/native-DOM diagnostics locate collection/native hit testing;
the original failed aggregate and unchanged isolated failure remain retained.
A separately hashed30s functional observation proves5000rows plusUnavailable;
it is explicitly not a performance or maintained-deadline pass. No hit test,
product deadline, frozen performance limit or candidate count was weakened.


Capture coverage follow-up: `proof-object-capture-negative-slot-1`6pass/0fail
executes complete-ambiguous slot search binding, not just successful hops. Its
producer bytes equal the35-pass witness regression. The role Rust snapshot's
`receiver-roles-engine-2` passes726Engine tests plus strict/static checks; its
native application step remains pending. These remain scoped component records.

## P05 native original object graph sealing

`PolicyProofObjects` exercises the fixed native broker probe against genuine
native receipts and real producer batches. It covers original raw annotation
binding (including harmless JSON reencoding), rule/reference substitution,
root and direct-slot replacement, raw excluded items, proxy related objects,
original scope identity/current containment, private child-scope document chains,
unknown facts, batch/stop retirement and independent annotation/scope byte caps.
Observer flags are deliberately masked in relationship-mutation cases so the
identity checks themselves must reject. This proves graph association only.

`object-graph-native-first-1`:13pass/0fail/0skip with exact source postcheck.
Review found an unintended512-parent native scope cap. A real three-frame scope
with180ordinary ancestors per frame reproduces it in
`object-graph-depth-red-1` (13pass/1fail); `object-graph-depth-fixed-1` passes14
after traversal uses the existing shared100000check budget. The expanded
`object-graph-depth-positive-3` passes15, including both a deep scope with a
Top-only complete negative and a Top scope containing a deep positive target.
It pins the qualified native binary and verifies its relevant C++ input hashes,
archives exact current sources and checks both afterward. Earlier positive-test
fixture failures remain in positive-1/2/diagnostic-1; moving an iframe ancestor
retired its child documents, and an unqualified iframe selector also matched an
existing outside frame. The final authored class selector is unique at each hop
and retains the outside frame as a scope/uniqueness control.

`receiver-roles-application-2` is terminal32pass/1fail, not green. The unchanged
stable-only replay passes, without explaining or erasing that aggregate failure.
New object application tests and real BrowserUi scope-handoff tests await shipping
execution. Production start/refresh/foreign-world transport and P05 exit remain
incomplete; none of these component results closes a P05 checkbox.

Foreign graph follow-up: `object-graph-foreign-first-1` passes17focused checks,
zero failures/skips, with pinned native inputs/binary and exact source postcheck.
Two tests add a genuine localhost foreign frame receipt and local shadow graph:
world mismatch, masked host replacement, loss of selected focus, independent Top
graph admission afterward, and tracker cleanup in both worlds. This is local
native sealing coverage; shipping collection/refresh and cross-world merging
remain pending. Frozen engine2/application qualification predates these two
QML-only tests and must retain its own source attribution.

## P05 scoped proxy containment

`PolicyProofObjects` now exercises ordinary actions under a genuinely acquired
region lease, without requiring an Interaction proof to enforce that lease.
Checkbox/radio labels whose related control starts outside are excluded; pairs
inside still click successfully. Moving an admitted control outside invalidates
both freshness and action, and leaves its checked state unchanged.
`scope-proxy-red-1` preserves18passes/four meaningful failures;
`scope-proxy-fixed-1` passes22checks with zero failures/skips. These records pin
the qualified native runner and matching relevant C++ sources, and retain exact
source inventories before/after. `scope-proxy-ordinary-1` additionally passes all
115checks across the six ordinary quality/action/control/geometry/outline/
presentation suites, with zero failures/skips and an exact source postcheck.
Independent source review found no blocker in admission or shared fresh/action
validation. These are component checks; shipping refresh and P05 exit remain open.

Root/raw-witness follow-up: `scope-proxy-root-witness-1` retained109passes/one
failure at the old detached-frame observer-count oracle. A focused diagnostic
shows exactly three live Top-owned roots: Document, catalog ShadowRoot and its
assigned slot. The updated test checks those exact identities/windows, removal
of the retained child's document/shadow/window/candidate registrations and no
live dirty/revision change after detached-object mutations. Independent review
verified the stronger test and `root-retirement-fixed-1` (3/0/0) source/archive
binding. `scope-proxy-root-witness-2` then passes110checks, zero failures/skips,
with exact source postcheck and pinned native runner. The existing5000-candidate
proxy-loop maintained10s deadline failure is explicitly excluded from both broad
runs and retains its earlier red evidence; these results do not qualify that gate.

## P05 explicit entered-Document proof

`site-policy-http/tst_entered_document.qml` exercises the independent producer/
native-receipt boundary. It does not mint the application's entered-frame issuer
or qualify its stopped capability-menu journey. The producer restricts ordinary
rows and policy targets to the exact native-selected same-origin Document,
intersects genuine Element leases, preserves Top identities/viewport projection,
and shares inspections, predicates and composed traversal depth. The separate
native `EnteredDocument` receipt seals original weak Document/host identities
and the native token/origin chain even for Top-only negative wire graphs.
An explicit entered Document may contain a focused descendant; terminal focus
remains a separate focused-root obligation. The real file-origin case logs
`file://` and executes a positive native seal, without claiming file-page
Interaction policy support.

Preserved reds include ignored metadata/Top escape (`entered-producer-red-1`,
2/4), stale unentered rows after failed entered preparation
(`entered-preparation-red-1`,2/3), and an undercounted focused root under
ShadowRoot→iframe (`entered-focused-depth-red-1`,3/1). The closed preparation
sentinel covers early budget/root/selector rejection and runtime resolution/
geometry failure. Independent native tests use genuine receipts/weak objects
with explicit private-graph corruption: the retained older b141438f… native
runner accepts physical depth nine and combined outer/primary slot-hop depth
nine (`entered-native-depth-red-1` and `entered-native-primary-depth-red-1`,
each2/1). Their records identify the older compiled probe separately from the
newer unbuilt source; these are not executions of the fixed probe.

`entered-depth-native-1` rebuilds the fixed probe/broker/runner and passes51/0/0.
`entered-primary-depth-final-1` passes54/0/0 against the same pinned native binary,
including positive depth8, negative depth9 and independent authored slot-hop
composition. `entered-ordinary-1` passes115/0/0 across the six ordinary Hint
suites. Source inventories/postchecks match; relevant compiled inputs and binary
SHA `7b0a59c17d8c3f5141b818f17b414ba747934a4b5646cd86817a6438023d0bce`
are pinned. Independent reviews approve these scoped fixes and evidence.

The larger single-process selected HTTP attempt timed out at180s after319passes
(`entered-http-regression-1`); it remains failed. The maintained fixture runner
now supports an existing-basename-only `--test-file` and runs independent files
serially by default, preserving180s per process and every per-test/native/
performance deadline. `entered-http-files-1` passes339selected checks across12
files, zero failures/skips, before the final three test-only depth additions.
The final54 record covers those additions with unchanged production/native code.
This qualifies per-file functional execution, not aggregate cross-file lifetime
or the explicitly excluded existing5000-candidate deadline failure. The stopped
71/5 shipping-entry gate, Rust consumer, original start/refresh/per-world transport
and P05 exit remain incomplete. No P06 ownership or activation is enabled.

## P05 raw facts before native sealing and classification

`raw-staging-red-1` preserves two failing Engine regressions against the previous
implementation: raw completion was accepted, and a structurally valid strict
claim was rejected by its predicate before native association. The new owned
raw-fact state retains the original request, candidates, graph and work counters;
classification consumes it once between the first native validation and final
original-graph validation, under the same deadline. It publishes no Hint session.

`raw-staging-engine-2` passes737 Engine tests, zero failures and one existing
S17 performance opt-in ignore, plus strict five-crate all-target Clippy,
format/fuzz-format/diff and KDE-free policy checks. Its corpus/differential unit
tests cover owned/immediate equivalence, incomplete prefixes, original reference
retention and exhausted work. This is not coverage-guided fuzz qualification.

`raw-staging-native-2` passes61 application ownership/classification/observation
checks, zero failures/skips, against the rebuilt shipping Engine/browser.
`objects-raw-seal-staging` observes four graph probes (two per broker validation);
`objects-strict-nonmatch-staging` observes only the first pair before rejection.
The first native run's59/2 result remains failed evidence: its two new assertions
incorrectly expected one probe per broker call. Correcting that test-only count
preserved the exact Engine/browser hashes. Both final records have exact2316-file
source postchecks; their sole source difference is the native counter correction.
This index was updated afterward. Per-world merge/projection, aggregate budgets,
shipping start/refresh and the stopped collection-entry gate remain required P05
work. No phase checkbox or exit gate closes here.

## P05 deferred ordinary and policy extraction

`site-policy-http/tst_deferred_collection.qml` tests the optional deferred
collector and one-shot `hintFinishInteraction` against real WebEngine/native
receipts. Ordinary candidates are finalized before policy scans. Suspended
inventories use weak DOM references and bounded scalar parent indices; live
traversal closures and style caches are released. Resume preserves the original
plan/nonce, candidate indices, selector revision, scope/entered identity and
work totals. Geometry and charged parent ancestry are checked again.

Preserved causal reds are `deferred-producer-red-1` (2/2; immediate annotations)
and `deferred-review-red-1` (2/3; late completion destroying replacements and
repeated ancestry calls). Focus diagnostics retain a correctly rejected stale
Top receipt; the final fixture establishes DOM focus before its original capture.
`deferred-compatibility-1` passes494/0/0: 379 selected HTTP checks across13 files,
including37 deferred cases, plus115 checks in six ordinary Hint suites. All19
commands pass with an exact2317-file source postcheck and the unchanged pinned
native broker/probe binary. The existing5000-candidate timing gate is explicitly
excluded. This index is updated after that source freeze.

Positive native seals cover Top, Element scope, shadow/slot paths, same-origin
focused/entered Documents and a foreign world. Negatives cover replacement,
mutation/navigation/stop, slot ABA, CSS animation without mutation, scope/world
substitution and remaining-budget exhaustion without refunds. Shipping asset
qualification, actual two-world orchestration/merge/projection, the aggregate
browser ledger and the stopped capability-entry journey remain incomplete.


## P05 retained worlds, original plans and eligible fragments

The ordinary coordinator/decoder pair `collection-work-payload-engine-2` and
`collection-work-payload-native-1` qualifies an identical2321-file snapshot:
758/0/1existingignore Engine and66/0/0 native, with strict/static checks green.
It predates the new raw-policy API. `policy-world-native-red-1` is3/1/0 at that
API's missing-method assertion; `world-policy-local-2` is132/0/0 for actual pure
modules/dependencies, excluding Qt/capture wrappers. New native original-plan
binding and the `work-policy-forged-plan`/optional-focus-loss rows are authored
and reviewed but still need their own runtime qualification. The unchanged
foreign row supplies the positive control; graph identity alone does not prove
which selector plan produced it.

`policy-plan-producer-1` retains37/0/0 deferred producer tests, including original
plan retention, plus a separate failed large-case lookup. Corrected
`policy-plan-producer-large-2` is4/0/0 with456185/543865UTF-16 collector scripts.
These use the exact historical native runner and qualify loose JS only.

`site-policy-http/tst_fragment_evidence.qml` now also checks bounded eligible
alternatives with original getClientRects indices. They are appended after
existing action-specific fragment hits; display fragments remain unchanged.
`eligible-fragments-red-1` is15pass/7fail at the missing-list assertions;
`eligible-fragments-green-1` and the strengthened clip/focus controls in
`eligible-fragments-green-2` each pass22/0/0. Exact source postchecks and the
historical runner pin are retained in each record. These are producer facts,
not a trusted native projection catalogue: capture/budget charging, exact
pre/post snapshot sealing, final inverse-crop local hit checks, native parent
point checks and immutable projected merge remain required P05 work. No phase
item or exit gate closes through these component results.


## P05 original-plan qualification and annotation reservation debit

`policy-plan-native-isolated-3` supplies the clean production raw-policy/original
plan result:70/0/0 on the exact saved application/browser artifacts and2321-file
source map from qualified-1. That earlier run's application portion overlapped
a producer exploration; its later QML25+32+37+22=116 checks ran separately.
Independent audit verified the isolated rerun's artifact pins and archive.
This qualifies that snapshot only; catalogue integration remains later P05 work.

The producer now retains one frozen primitive fragment snapshot per raw-referenced
candidate, prunes discarded entries without refund, and retains its charged
reservation debit through successful and failed extraction. Catalogue and raw
annotation share the UTF-8 cap. Focused reds and exact/minus-one ASCII/Unicode
controls are in `fragment-catalogue-*` and `fragment-debit-*`. The final
`fragment-catalogue-compatibility-1` passes158/0/0 across proof25, entered32,
deferred37, fragment22 and raw-witness42 cases; the protected5000 timing red
remains excluded. Native catalogue identity/geometry/projection is not proved.

The Rust ledger requires policy annotation_bytes, rejects it on ordinary stages,
charges its original-grant-bounded value once, and burns annotation allowance on
any matching unknown rejection. Focused ledger red133/2 and ordinary-failure
red136/1 are preserved; `policy-debit-ledger-green-3` passes137/0/0 on actual pure
sources/dependencies, excluding capture wrappers/native. The native broker now
compares this declared debit with the original private counter and checks the
combined byte floor; `native-debit-guard-red-1` is2/4 on the preguard broker,
while clean-build green-2 passes6/0/0. Green-1 is a stale-CMake-object link failure,
not a test result. Full updated Engine/static checks are running separately.
This byte guard is not a native fragment catalogue receipt or P05 exit approval.


P05 captured native fragment catalogue checkpoint (12 November 2026):
`policy-debit-integration-engine-1` (769/0/1 existing opt-in ignore) and matching
`policy-debit-integration-native-1` (71 application + 120 QML, no failures/skips)
are terminal and independently approved for the prior debit snapshot. Source
maps match exactly (2,321 files); archives/artifact pins verify. The underreported
production case proves a positive private debit larger than raw UTF-8 bytes,
changes only the declared debit, then asserts rejection and exhausted budgets.

New opaque `FragmentCatalogue` binds the original native world, raw graph,
request plan/stamp/revision/scope, viewport/debit and geometry from retained
ordinary candidates. Its two graph probes return the same exact private string
around native postchecks. The fixed probe checks final raw membership, identity,
kind/policy, primary/index and eligible geometry against original ordinary bounds.
Native clear cancels pending callbacks; UI owners must retire work on epoch loss.
This retains captured facts, with no fresh geometry, parent exposure or action
permission claim.

`native-catalogue-integrity-red-1` is 2 pass/20 intended failures on the immutable
pre-catalogue native runner. Real graph/debit controls pass before each bad
catalogue is incorrectly accepted. `native-catalogue-integrity-green-1` builds
successfully and passes37 with one test-setup failure: the whitespace snapshot
control exceeded its byte floor. Only setup changed to reserve headroom before
capture; `native-catalogue-integrity-green-2` reuses the exact built native pin
and passes38/0/0. Controls include raw-target/cache corruption, canonical-index
ties, ordinary bounds/viewport, replacement/namespace/clear, historical geometry,
Top/shadow/same-origin/wrapped/excluded/empty outcomes. Exact source/archive and
native-source/pin checks are recorded. The pure actual-source geometry/viewport
harness `catalogue-original-geometry-local-2` passes137/0/0 with no dependency
delta, after correcting a wrong expected rectangle in local-1 against the
actual policy fixture. Historical failures remain preserved.

These focused checks exclude compilation/runtime of the new capture wrapper,
FFI and adapter wiring. Full Engine/native integration, trusted frame projection,
composed depth, shared verification work, injective merge, original ordinary
producer-input binding and shipping start/refresh remain required. Every P05
checkbox and gate remains open; P06–P17 remain pending.


P05 composed-depth primitive checkpoint (13 November 2026): native frame-role
identity remains `depth`; separately checked `composed_depth` includes ancestor
open-shadow roots and is required before a foreign ordinary depth grant. Rust
keeps native document association distinct from traversal limits, including
focused Top, attempted root hops and strict/observed collection-primary paths.
`composed-depth-local-red-1` preserves137 prior passing cases and reproduces four
failures (extra grant level, invalid authority, and both Top/foreign depth-nine
roots accepted). The same tests pass141/0/0 in local-green-1, exact218-file source
postcheck with no dependency delta; independent source/evidence reviews approve
that pure slice.

The native-only ancestry capture correlates each retained native child before/
after exact parent active-host capture, counts actual ShadowRoot ownership and
retains bounded weak host/root/assignment dependencies. Forwarded/manual slots
are dependency edges without changing the frame's actual depth. Mutation records
permanently retire ABA paths. Receipt retirement, stop before ordinary tracking,
clear and broker destruction release observers/listeners across parent worlds.
Top extraction does not inherit optional selected ancestry.

`composed-depth-native-1` configures but fails compilation on two non-const Qt
frame calls; it has no runtime result. Local frame-wrapper copies correct those
calls. Native-2 configures/builds/tests successfully (14/0/0). Native-3 uses the
same verified8652062eb640ee95735037901080ec677852a82820d670e96f7ca025de239dd2
binary and strengthens only QML: depth-eight rows seal the real native graph;
a false builder base depth produces a real live target but native sealing rejects
global depth nine from retained ancestry. It passes15/0/0. Both records have exact
2,324-file source postchecks; archives, pin and five native inputs independently
verify. Slot/forwarding/host ABA controls preserve fresh native frame/doc/focus or
geometry while rejecting old ancestry. These are native primitive checks, not
full adapter/FFI or global-work qualification. The latest added native-role
finalizer unit cases still need full Engine execution.

Prior catalogue integration `native-catalogue-integration-engine-1` passes769
Engine cases (one existing S17 ignore) and all six checks, including compilation
of that snapshot's wrapper/FFI/adapter. Its matching production qualifier
`native-catalogue-integration-native-1` is running on frozen Validation, excluding
new ancestry. Native ancestry inspection/verification and cleanup dispatches
still require shared ledger/outstanding-work integration. Trusted fragment
projection and original shipping transport remain open; every P05 gate remains
unchecked and P06–P17 remain pending.


P05 retirement and integration follow-up (13 November2026): the prior catalogue
production snapshot now passes71 application +152 QML cases, all10commands0;
paired769-test Engine map/archive/pins are independently verified. This closes
only that snapshot's catalogue wrapper/FFI/adapter integration.

Retained-realm regression `ancestry-retirement-native-red-2` proves a live native
two-frame chain then real parent navigation leaves an owned observer active in
the retained old Document:disconnects0/records1. Red-1's earlier MainWorld-expando
setup error remains separate. Native-4's pagehide retirement passes17 normal
navigation/detachment controls; Native-5 adds a prior page handler that suppresses
the late hook and reproduces the same leak (17pass/1fail). The shared retirement
hook now runs at DocumentCreation in ApplicationWorld/subframes through the
existing production focus-observer installation. It creates no ancestry map,
observer or closed-Hint scan. Native capture requires its marker and has no late
injection fallback. Native-6 passes18/0/0 after rebuild; Native-7 reuses the exact
cc9a58d9e10755227ff028725b9ac5bcaec6889c6df75f36a8ef1ce0bd4f7cbf
pin and adds missing-marker fail-closed/no-observer assertions, passing19/0/0.
All historical failures are intact. Source/evidence reviews approve the lifecycle
primitive; full current-source Engine/native binding and shared native-work/
cleanup-dispatch accounting remain pending. P05 stays open.


P05 original-ledger/cleanup follow-up (13 November2026): ancestry's paired frozen
2,325-file integration is now green:774 Engine tests/one existing opt-in ignore,
all6checks0;71 application +171 QML rows, all11commands0. These independently
reviewed maps/archives/logs exclude the following newer source.

The boxed original-request ledger and bounded structural decoder have a pinned
actual-source pure run `original-ledger-local-4`:157/0/0, dependency delta empty,
exact220-file source postcheck. Local3 also passes157 but resolved11 dependency
versions differently and is not pinned qualification. Native reservation tokens
bind ledger/request/operation/phase/stage/world; unknown matching completion
burns capacity and late replies cannot refund. Both structural staging paths use
the remaining original-request verification allowance before execution. Exact
allowance, one-less, zero-before-parse and two-world shared-boundary regressions
are included. Native capture/FFI still require a successful full Engine run:
original-ledger-integration-engine1 stops at strict Clippy's unnecessary semicolon,
now corrected, with its failure preserved. The graph/decode1m allowance remains
only a subtotal; production preflight grants are deliberately closed pending the
complete native operation table and real reports/charging.

New native cleanup dispatch reserves within128 total before installing ancestry
observers, coalesces one parent document's records, retains state across broker
cancellation/destruction and returns capacity only after strict success or
confirmed native retirement. Failed/unknown cleanup retains its slot without a
retry loop. `cleanup-dispatch-native-red-1` runs six new negative rows on the
immutable previous ancestry native binary:2init/cleanup passes,6 intended fails
on missing reserved/burned capacity after authentic observer positives. No missing
API/setup failures. Green1 rebuilds actual broker/probe with the exact SDK and
passes all26 composed-depth rows, no failures/skips, all3commands0, exact2,327-file
source postcheck. This includes saturation, shared-parent receipt release/clear,
full-capacity capture refusal/no observers, broker destruction with surviving
receipt/native frame and deliberate release exception retaining one slot with
no retries. The injected exception's expected JS critical remains in both logs.
Source review found and corrected a callback-only acknowledgement bug before
green; a native callback alone cannot prove observer release. Full Engine,
application/other QML compatibility and request-wide native charging remain
pending. No P05 checkbox/gate closes and no P06 work starts.


P05 finite attachment scheduling follow-up: attachFrame now preserves its
original absolute deadline through at most one stale-document retirement and
initialization, rejects unknown cached probes without retiring a valid owner,
and checks expiry after observable leasesChanged notification. Focused-child
scan/attach share that deadline. Source review approves these bounds; the
post-notification check was added in response to its concrete finding.

Bounded-attachment-native-red1 is preserved as a harness permission/setup error,
not causal evidence: copyfile did not preserve the saved runner execute bits.
The bytes/hash match the earlier cleanup pin; execute metadata was restored and
future helpers use copy2. Red2 on that unchanged prior native pin passes3controls
and fails3actual required boundaries:650+650ms combined work and1100ms signal
notification still return owner success, while a full dispatcher's unknown cached
probe loses the valid original owner. Green1 rebuilds the new broker and passes
the entire24-case frame-receipt suite, no failures/skips; configure/build/test0,
exact2,327-file postcheck. Existing timeout-capacity, navigation, clear/reentrant
and destruction cases remain present. This is a finite attachment/dispatcher
primitive, not full native schedule or request-wide work accounting.

Combined original-ledger-cleanup-integration-engine2 stops at strict all-target
Clippy's two unreadable numeric test literals. They are changed from100000 to
100_000, without altering assertions. Its failure remains preserved; no Engine
runtime result is claimed for the current ledger yet. All phase gates remain open.


P05 ledger/cleanup/attachment paired integration is terminal and independently
reviewed: Engine3 passes793/0/one existing opt-in S17ignore, all6commands0;
matching native3 passes71application +202QML (61proof/32entered/37deferred/
22fragment/26composed/24frame), all12commands0, zero failures/skips. Exact2,327-
file maps/archive contents/postchecks and saved QQ pin match. Large artifact
hashes were inspected in records, not independently recomputed. This excludes
all newer original-ordinary-input changes and the stopped chooser gate.

New original ordinary input is frozen before producer admission and ordinary
Budgets freeze at stage activation rather than being reconstructed later. The
private Rust getter derives original selectors/full snapshot, family/proxy/lease,
Top target-search facets, deferral and issued Budget, independently of small
metadata caps. The immutable string survives deferred extraction and resides
readonly in the final private batch. Native catalogue capture compares it with
Rust's original value and retains that expectation for reseals. This seals query
provenance after execution; full native ticket admission/charging before work
still requires the source-derived protocol integration.

Ordinary-input-local1 passes158 pure tests, pinned dependency delta empty, exact
220-source postcheck. This adds original-grant immutability/pending-native wait/
current-remainder rejection and checks the retained grant after both ordinary
worlds have spent work. It excludes the new Rust getter/FFI/Qt path.

Ordinary-input-native-red1 uses the immutable previous attachment native runner
with unchanged test API:5passes (init/cleanup plus3compat controls),10intended
accepted-invalid failures. Each negative first proves real ordinary/raw/native
evidence; selector substitution keeps the revision but actually selects Alternate.
Family/proxy/facet/deferral and all5grant changes are likewise accepted by old
native sealing. Green1 has no runtime: configure0/build2 exposes missing explicit
QJsonObject include in the new test-only carrier. Direct QJsonDocument/QJsonObject
includes fix the translation unit. Green2 rebuilds actual broker/probe/helper and
passes all75proof cases,0fail/skip, all3commands0, exact2,327-file postcheck. Valid
unchanged/smaller-policy-resume/>4KiBselector input cases pass, as does a genuinely
acquired Element lease's native scope seal/catalogue/reseal before omitted/wrong
lease producer rejection with0candidates. Expected fixtures use test-only plan
carriers; production expects only the original private Rust value. Its new actual-
capture Rust test and full Engine/production API integration remain unqualified.
Every P05 box/gate stays open; no P06 starts and the stopped71/5gate is untouched.


P05 original-input full Rust integration is terminal: ordinary-input-integration-
engine1 passes795/0/one existing opt-in S17ignore and all6checks0, exact2,327-file
archive/postcheck. Independent review verifies all archived sources and both
new getter/frozen-budget tests. This qualifies the unchanged Rust/bridge/JS input
implementation, not newer C++ behavior.

The remaining repeatable public document validate entry now admits once per
original native handle, before dispatch. A wrong handle/adapter/currentness check
does not consume the gate; matching timeout/failure does. Its flag is checked
after observable currentness reads to cover reentry. The stable real native
ownership fixture first requires successful validation, then requires synchronous
second-call refusal and unchanged dispatch occupancy. Source review approves
this finite scheduling correction; it has not run yet. The current delta from
Engine1 is exactly adapter.cpp/header and this application fixture, with identical
Rust/bridge/JS hashes. Independent review approves reusing those unchanged Rust
results alongside a rebuilt native archive/browser and full selected application/
six-QML qualification, with explicit newer C++ source scope. No P05 exit follows.


P05 original-input/finite-validation native integration is now terminal and
independently reviewed: ordinary-input-validation-integration-native1 passes
71application +216QML (75proof/32entered/37deferred/22fragment/26composed/24frame),
all12commands0, zero failures/skips, exact2,327-file archive/postcheck. The stable
ownership row actually executes the successful-first/synchronous-second refusal
and unchanged occupancy assertions. Its source delta from qualified Engine1 is
only the three reviewed C++/header/test files plus this inventory; Rust/bridge/JS
hashes match. Cargo/CMake linkage and saved QQ pin verify. Large binary/library
hashes were inspected in records, not independently recomputed. This qualifies
production original-input sealing and finite public validation, not shared native
charging, projection, shipping or the stopped chooser gate.

New Primary-only fixed native operation schema replaces caller-chosen integers
at the private reserve API with kind/checkpoint/document/edge/child/pass keys.
The reserved ranges end at31,058; only supported sparse positions can encode.
They cover collection-staging preflight/policy identities, with4graph probes per
world. Ordinals identify budget operations and must never select native targets:
actual dispatch still needs the captured native catalogue/topology context.
Production GraphDecode profile funds only verification-only graph keys up to the
existing100k local allowance; metadata/projection remain unfunded. Arbitrary-ID
bitmap testing is cfg(test), and the production primitive is private. Prototype
funding is cfg(test) only. Existing public post-bind/legacy/projection operation
coverage and complete numeric schedule/actual work reports remain unimplemented.

Fixed-native-slots-local1 and local2 each pass162/0/0, dependency delta empty,
exact222-source postcheck. Local2 adds private production reservation/test-only
integer adapter and the ordinal-authority boundary documentation. Independent
review verifies both archives, explicit exhaustive identity/range/invalidJSON/
world-phase/profile-closure and8graph+2decode subtotal tests. The new actual-
capture service integer-ID rejection assertion is outside this pure harness and
still needs full Engine execution. No native dispatch or P05 exit is inferred.


Fixed native operation integration is now terminal: fixed-native-slots-integration-
engine2 passes799/0/one existing opt-in S17ignore, all6commands0, exact2,329-source
postcheck. The original_native_ledger test executes the integer65535 refusal
and unchanged pending state in the actual capture service. Independent review
verifies the complete archive and frozen Validation sources. Engine1 is preserved
as interrupted-harness after a tool-runtime reset; its log finished but original
exit is unavailable, so it is not counted as successful qualification.

Retained-carrier P05-12 component: the actual renderer-held annotation is bounded
in UTF-8 before parsing, including declared live-byte debit. The native script
captures annotation/catalogue once and returns that verified catalogue. Source
review approves this fix. Immutable old runner red1 records4pass/4intended fails:
3oversized carriers accepted and forged catalogue fetched4times; exact-cap/small
spelling positives pass. Rebuilt current native green1 passes81/0/0 proof-object
cases, all3commands0, exact2,329-source postcheck. Shared native work accounting
is still incomplete. Broader production/native qualification follows on this
frozen snapshot; no P05 item or gate is closed by the component result.


Primary-only graph-ticket integration now supplies request-local weak original-
handle callbacks to the actual native fragment capture/reseal dispatches. The
private typed Rust graph keys use0/1 for capture and2/3 for reseal. Native dispatch
retains settlement independently of user callbacks and validates fixed-shape
bounded graph reports before returning results. Native NoScript is possible
only before runJavaScript; timeout, cancellation, destruction or unreadable
reports burn the queued grant, with late completion unable to refund. A failed
ledger completion suppresses an otherwise valid catalogue. Source review found
and verified fixes for retired-frame ticket loss, stale-generation refunds,
reentrant deadline extension and temporary-test-broker cleanup. Runtime proof
and actual adapter/Rust progression are still pending.

The frozen graph-ticket-native-red-source has2,330 sources, a native ticket type,
unused FragmentRequest field and8real-broker negative regression rows, without
broker consumption. Broader green adds dispatched timeout, broker destruction
and rejected-settlement regressions; earlier81proof-object cases stay intact.
The only live heavy job is retained-carrier-integration-native1 on frozen
Validation; graph red/green will run sequentially afterward. This component meters
only the existing graph loop; carrier/JSON/canonical comparison, document/provider
probes, trusted projection, complete native schedule and shipping remain open.
Every P05 checkbox/gate and P06–P17 remain unchecked.


Graph-ticket native component is terminal and independently audited. Red1 is a
preserved source-inventory setup failure, with no build/test. Unchanged frozen
red2 builds successfully and fails8intended cases (2init/cleanup passes), exact
2,330-source postcheck. Green1 builds the actual broker and passes92/0/0 full
proof-object cases, all3commands0, exact2,330-source archive/postcheck and saved/
commanded QQ pin552004…a4d0. All11new graph rows execute: four refused passes,
settled capture/reseal and repeat refusal, charged rejection, pre-dispatch expiry,
clear, destruction, dispatched timeout/late reply and refused ledger completion.
Source and evidence reviews approve this broker-with-native-test-context scope.
The actual Rust adapter wiring has not yet been compiled/run by that standalone
QQ target; production qualification follows on a newly frozen Validation snapshot.
This component does not qualify the full native work schedule or close P05.


Graph-ticket production native1 is preserved FAILED:71application passed, but
stale standalone broker object caused81proofpass/11graphfail; remainingQQ suites
never ran. Independent symbols/mtime/build-log audit identified copy2 timestamp
reuse. The corrected native2 asserts identical2,330-source dictionary and uses
fresh per-run application/QML CMake directories. Native2 is terminal and audited:
71application +233QQ (92proof/32entered/37deferred/22fragment/26composed/24frame),
all12commands0, zero failures/skips, exact archive/postcheck and saved/build-copy
artifact hashes. Fresh QQ compile log and symbols prove the new broker; actual
production Top→selected policy flow goes through original Rust reservation/
completion with all8graph slots. Stopped chooser remains excluded. Future source
sync copies content/mode with a fresh timestamp; copying old mtimes is not valid
compilation evidence.

New document-probe headroom remains a prototype, not shipping receipt metering.
Its supported inventory is76preflight+62Top+126selected+18retained validation
=282; plain Top bind/fallback/before/after use onlydocument0. A separate18-slot
validation range is appended after all old graph/collection IDs (end31,076).
Stage0Validation is recognized but explicitly denied by the actualcapture service
until a native-committed binding exists. Only test constructors/profiles fund
these probes; production GraphDecode reserves zero headroom and stays closed.
The classifier itself is compiled in production but grants no funding authority.

The shared ledger now protects unissued probe units within the existing50k pool,
with current/frozen producer checks and NoScript unit refund without slot reuse.
Red1:164pass/one intended50,000-versus49,718 failure, deps[], exact222postcheck.
Green1:168/0/0, deps[], exact222postcheck, including full-cap producer/headroom
conservation, all282positions, wrongunits, valid-unfunded native kinds and
validation boundaries. Test factoring occurred before green's first execution.
Source review approves; independent evidence audit and fullEngine/strictClippy
follow. Pureharness excludes the actualcapture uncommittedValidation assertion.
Native document reports/context, original catalogue binding, provider/ancestry/
other native work, trusted projection, canonical merge and shipping remain open.
P05 and all later gates remain unchecked.
