# Ferric Versioned Browsing — Implementation Slices



---

## Appendix B. Dependency-ordered implementation slices
The companion JSON is the machine-readable execution ledger. Paths labelled proposed do not exist merely because they appear here. Each slice also inherits the global quality, privacy and ownership rules in Sections 4, 23 and 25. A negative qualification result can complete a research slice while leaving dependent production capability tasks blocked.

### P0 — Baseline, decisions and probes

#### VB-000 — Reconcile the working checkout with the inspected baseline
**Dependencies:** none  
**Design sections:** 3, 25  
**Type:** baseline; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `CONTRIBUTING.md`
- `Cargo.toml`
- `docs/DEVELOPMENT_SPEC.md`
- `docs/requirements.csv`
- `crates/ferric-browser-engine-qt/build.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/baseline.md`

**Implementation:**
1. Read all applicable repository/agent instructions and record git HEAD plus the main comparison without overwriting unrelated work.
2. Verify the ownership map, actual migration mechanism, command-name collisions and current xtask fixture entry points. Record any renamed symbols before dependent coding.
3. Record installed Qt/QtWebEngine/Chromium versions, available native display tests and the dependency cache. Do not install missing tools implicitly.

**Tests:**
- Run the smallest available existing pure test group and record baseline failures separately from future changes.
- Check that no user profile, credential file or production account is selected as a test fixture.

**Done when:** A reproducible baseline record exists and every mapped path used by the next slice is confirmed or corrected. Missing environments and pre-existing failures are recorded rather than hidden.

**Evidence:** `docs/testing/versioned-browsing/VB-000.md`.

#### VB-001 — Record capture, branch and cookie capability ADR decisions
**Dependencies:** VB-000  
**Design sections:** 4, 20  
**Type:** decision; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/architecture/ADR-0006-profile-and-session-durability.md`
- `docs/architecture/ADR-0011-context-profile-window-actions.md`
- `docs/architecture/ADR-0012-journey-and-ephemeral-state.md`
- `docs/architecture/ADR-0014-explicit-scroll-target.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/architecture/<next-number>-versioned-browsing-boundaries.md`

**Implementation:**
1. Assign the next available ADR number; document the separate explicit capture namespace and unchanged safe recovery.
2. Specify window/profile affinity, transient export consent and the prohibition on treating a mirror as a strict cookie snapshot.
3. Record maintainer acceptance or the existing authorisation basis. Do not label an unapproved privacy exception accepted.

**Tests:**
- Review each proposed ADR exception against VB-I01 through VB-I18.
- Ensure the ADR explicitly keeps private automatic persistence and cross-profile live reparenting prohibited.

**Done when:** Implementation has an explicit architectural authority for the new normal-profile capture path. Conditional identity and explicit branch export decisions are separately visible.

**Evidence:** `docs/testing/versioned-browsing/VB-001.md`.

#### VB-002 — Add the qualification record and capability vocabulary
**Dependencies:** VB-000  
**Design sections:** 6, 7, 23  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/diagnostics_snapshot.rs`
- `crates/ferric-browser-engine-qt/src/lib.rs`
- `crates/ferric-browser-core/src/action.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/capabilities.json`

**Implementation:**
1. Inventory the actual diagnostics fingerprint APIs and add a versioning qualification record format.
2. Define supported/unsupported/unqualified plus scope, backend and evidence IDs; initialise new native capabilities unqualified.
3. Represent general Qt cookie identity/completeness gaps explicitly. No numerical mirror revision is an engine transaction ID.

**Tests:**
- Test capability record parsing and rejection of unknown schema/fingerprint mismatch.
- Assert a config flag alone cannot turn an unqualified capability into supported.

**Done when:** Every planned native feature can report a stable reason for unavailability. The record contains no cookies, URLs or user-identifying account data.

**Evidence:** `docs/testing/versioned-browsing/VB-002.md`.

#### VB-003 — Probe live freeze, retained ownership and resume
**Dependencies:** VB-000, VB-002  
**Design sections:** 12, 23  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`
- `docs/testing/m0-96-tab-suspension.md`
- `docs/testing/m0-157-tab-transfer-contract.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_lifecycle_probe.qml`

**Implementation:**
1. Create disposable fixtures with an in-memory sentinel, dirty editor, freeze/resume handlers and a background timer.
2. Retain and hide the actual view, observe lifecycle transitions, resume, and verify instance identity without claiming immutable state.
3. Exercise source host closure/reparenting in a disposable native window and record eligibility/refusal cases.

**Tests:**
- Verify sentinel identity survives the supported retain/resume path but no API can clone it into a new view.
- Verify freeze/resume handlers can mutate state, and invalid lifecycle requests are reported.
- Record Wayland and renderer-loss results separately from headless QML checks.

**Done when:** The supported ownership strategy is selected from actual runtime evidence. A negative probe prevents LIVE capability from being enabled but does not block structural work.

**Evidence:** `docs/testing/versioned-browsing/VB-003.md`.

#### VB-004 — Probe page-save routing and archive isolation
**Dependencies:** VB-000, VB-002  
**Design sections:** 13, 15  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeChrome.qml`
- `crates/ferric-browser-engine-qt/src/request_interceptor.h`
- `crates/ferric-browser-engine-qt/build.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_archive_probe.qml`

**Implementation:**
1. Save a local fixture as MHTML through the actual QML API into a disposable controlled path.
2. Observe page-save auto-start and public download correlation alongside an ordinary download.
3. Open the result in a fresh script-disabled, network-denied profile and capture attempted remote traffic.

**Tests:**
- Cancellation, interruption and ambiguous download association must not be called successful capture.
- Malicious archive resource/redirect/form/pop-up attempts produce no unauthorised network/native action.

**Done when:** Public API routing and the archive isolation boundary are evidenced, or the missing guarantee is a named blocker.

**Evidence:** `docs/testing/versioned-browsing/VB-004.md`.

#### VB-005 — Probe cookie representation and completeness without credentials
**Dependencies:** VB-000, VB-002  
**Design sections:** 17  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/request_interceptor.h`
- `crates/ferric-browser-engine-qt/build.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/cookie-api-gap.md`

**Implementation:**
1. Use only disposable profiles and synthetic cookie values to enumerate public Qt cookie fields and events.
2. Test host-only/domain variants, duplicate names/paths, expiry and partitioned-cookie exposure on the installed build.
3. Document whether any actual API establishes enumeration completion; do not turn a quiet interval or sentinel into a proof.

**Tests:**
- Assert unknown completeness is not empty state and identity collisions cannot be silently overwritten.
- Record Qt native exposure versus any debugging-only inspection separately.

**Done when:** The strict cookie snapshot/seed/apply capabilities have defensible initial status. A failed strict gate is an expected research outcome, not an invitation to read Chromium databases.

**Evidence:** `docs/testing/versioned-browsing/VB-005.md`.

#### VB-006 — Create reusable synthetic web and fault fixtures
**Dependencies:** VB-000  
**Design sections:** 23.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/xtask/src/main.rs`
- `.github/workflows/ci.yml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/fixtures/versioning/`

**Implementation:**
1. Extend the actual existing local fixture runner discovered in VB-000; do not invent a parallel packaging system.
2. Add deterministic pages for forms, frames, lifecycle callbacks, controlled inputs, archive requests and synthetic login/session coupling.
3. Provide disposable profile roots and test-only HTTPS trust where required; no global TLS or sandbox disable.

**Tests:**
- Fixture startup/teardown leaves no persistent user-profile data.
- Secure/SameSite cases run only with their actual transport prerequisites; unavailable cases report skipped/unqualified explicitly.

**Done when:** Later native slices can reproduce behaviour without live accounts or Internet access.

**Evidence:** `docs/testing/versioned-browsing/VB-006.md`.

#### VB-007 — Register feature requirements and execution evidence
**Dependencies:** VB-000, VB-001, VB-002  
**Design sections:** 23.4, 24, 25  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `CONTRIBUTING.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/README.md`

**Implementation:**
1. Add feature requirement IDs with the real CSV schema and repository status vocabulary.
2. Map requirements to proposal invariants and task IDs; distinguish release core from conditional extensions.
3. Install the evidence template and goal-mode handoff without marking future tests verified.

**Tests:**
- Validate CSV structure and repository documentation checks.
- Ensure every enabled-feature requirement has a test/evidence owner.

**Done when:** An agent can choose the next eligible slice and resume after interruption without rereading the entire conversation.

**Evidence:** `docs/testing/versioned-browsing/VB-007.md`.

### P1 — Pure domain model and command semantics

#### VB-008 — Introduce versioning identity newtypes and limits
**Dependencies:** VB-001, VB-007  
**Design sections:** 5, 21  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/id.rs`
- `crates/ferric-browser-core/src/model.rs`
- `crates/ferric-browser-core/src/lib.rs`
- `Cargo.toml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/mod.rs`
- `crates/ferric-browser-core/src/versioning/identity.rs`
- `crates/ferric-browser-core/src/versioning/limits.rs`

**Implementation:**
1. Add durable UUID v4 newtypes and explicit application/profile-instance identities using current dependency conventions.
2. Keep runtime TabId/ProfileId distinct from durable IDs; define logical page lineage identity.
3. Add validated initial bounds with no runtime environment lookup in core.

**Tests:**
- Test invalid/ambiguous IDs, equality/hash semantics and cross-instance identity mismatch.
- Test configured upper/lower bounds and UUID serialisation round trips in the DTO layer when introduced.

**Done when:** Core compiles/tests without Qt and cannot conflate a stored page with a live tab.

**Evidence:** `docs/testing/versioned-browsing/VB-008.md`.

#### VB-009 — Model fidelity, capabilities and redacted failures
**Dependencies:** VB-008, VB-002  
**Design sections:** 6, 22  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/lib.rs`
- `crates/ferric-browser-runtime/src/lib.rs`
- `crates/ferric-browser-ipc/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/error.rs`
- `crates/ferric-browser-core/src/versioning/capture.rs`

**Implementation:**
1. Add independent retention, representation, coverage, consistency, identity and persistence dimensions.
2. Create typed capability requirements and versioning reason codes mapped through existing IPC errors.
3. Provide only redacted display/debug projections for sensitive categories.

**Tests:**
- Test live+partial, archive+unavailable-form and lost+durable-fallback combinations.
- Assert unknown/incomplete states never serialise as complete or empty.

**Done when:** UI-friendly labels derive from structured facts without strengthening their meaning.

**Evidence:** `docs/testing/versioned-browsing/VB-009.md`.

#### VB-010 — Define pure scope resolution and captured owners
**Dependencies:** VB-008, VB-009  
**Design sections:** 5.4, 9  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/model.rs`
- `crates/ferric-browser-core/src/dispatcher.rs`
- `crates/ferric-browser-core/src/reducer.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/model.rs`

**Implementation:**
1. Define captured owner tokens and immutable tab/window scope projections.
2. Validate one-profile window scope, live target generations and native owner identity.
3. Leave context capture unavailable until its dedicated integration slice.

**Tests:**
- Test tab switch after admission, document replacement, wrong-profile target and closed owner.
- Test deterministic membership ordering and maximum scope size.

**Done when:** Delayed operations cannot be redirected by later focus or profile changes.

**Evidence:** `docs/testing/versioned-browsing/VB-010.md`.

#### VB-011 — Add branch lineage and working-state reducer transitions
**Dependencies:** VB-008, VB-010  
**Design sections:** 5.2, 5.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/model.rs`
- `crates/ferric-browser-core/src/reducer.rs`
- `crates/ferric-browser-core/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/model.rs`

**Implementation:**
1. Add branch records, parent references, working revisions and retained membership behind ApplicationState privacy.
2. Add preparation/publication/failure transitions without Qt or persistence work.
3. Reject cycles, duplicate live ownership and ambiguous line names.

**Tests:**
- Test parent-child-grandchild lineage, max depth, invalid cycles and delete-with-owned-live-work refusal.
- Verify immutable fork references do not change when working state evolves.

**Done when:** All versioning domain mutation enters through typed reducer events.

**Evidence:** `docs/testing/versioned-browsing/VB-011.md`.

#### VB-012 — Implement operation state and cancellation semantics
**Dependencies:** VB-010, VB-011  
**Design sections:** 9  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/reducer.rs`
- `crates/ferric-browser-runtime/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/mod.rs`
- `crates/ferric-browser-application/src/versioning/coordinator.rs`

**Implementation:**
1. Define typed operation envelope/states and application-owned scheduling.
2. Inject clock/IDs for deterministic deadline tests; reject late/double completions.
3. Add explicit Partial and OutcomeUnknown outcomes without implementing cookie writes yet.

**Tests:**
- Exercise cancellation in every state, owner close, duplicate acknowledgement and expired consent.
- Assert a late callback can clean only its own staged resources, not publish new state.

**Done when:** No operation relies on active focus or an unbounded pending callback.

**Evidence:** `docs/testing/versioned-browsing/VB-012.md`.

#### VB-013 — Specify typed command options and registry metadata
**Dependencies:** VB-009, VB-010  
**Design sections:** 8  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/command.rs`
- `crates/ferric-browser-core/src/dispatcher.rs`
- `crates/ferric-browser-core/src/action.rs`
- `crates/ferric-browser-core/src/completion.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/commands.rs`

**Implementation:**
1. Add VersioningCommand conversion from existing parsed tokens with the closed grammar in Section 8.
2. Register primary commands/actions and capability requirements without new side effects.
3. Add commit/stash/branch/merge-plan subjects to exhaustive registry handling.

**Tests:**
- Test quoted names, unknown/duplicate flags, counts, shell-like literal inputs and incompatible options.
- Assert UI/CLI/IPC conversions produce equivalent typed requests.

**Done when:** Command discovery is generated from the existing registry, not a hand-written menu/parser.

**Evidence:** `docs/testing/versioned-browsing/VB-013.md`.

#### VB-014 — Implement deterministic structural snapshots and diff
**Dependencies:** VB-010, VB-011  
**Design sections:** 10.2, 11  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/model.rs`
- `crates/ferric-browser-core/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/diff.rs`
- `crates/ferric-browser-core/src/versioning/capture.rs`

**Implementation:**
1. Project logical windows/pages and safe metadata into immutable inputs.
2. Compare logical identities, ordering and supported state categories with stable output order.
3. Keep unknown page/cookie coverage distinct from absent values.

**Tests:**
- Test same URL in different logical pages, reordered tabs, selected page, finite zoom/scroll and unavailable fields.
- Property-test diff symmetry classifications and bounded output where current test dependencies allow.

**Done when:** A structural diff describes user state, not runtime-ID churn.

**Evidence:** `docs/testing/versioned-browsing/VB-014.md`.

#### VB-015 — Implement the pure cookie three-way truth table
**Dependencies:** VB-008, VB-009  
**Design sections:** 17.2, 18.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/merge.rs`

**Implementation:**
1. Define a backend-neutral supported-cookie record with raw byte names/values and explicit identity fidelity.
2. Implement base/parent/child comparison for additions, updates, deletions and unknown states.
3. Inject evaluation time; never extend expiry or combine opaque token values.

**Tests:**
- Exhaustively test the Section 18 truth table including equal additions and converged deletions.
- Test byte values, attribute-only changes, expiry, unknown partitions and incomplete snapshots.

**Done when:** Planner output is deterministic and refuses unsupported/incomplete inputs; no native writes exist.

**Evidence:** `docs/testing/versioned-browsing/VB-015.md`.

#### VB-016 — Define immutable previews and confirmation permits
**Dependencies:** VB-012, VB-013, VB-015  
**Design sections:** 8.3, 18.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/action.rs`
- `crates/ferric-browser-application/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/projection.rs`

**Implementation:**
1. Bind confirmation permits to operation, exact scope, owner, security revision and immutable preview choices.
2. Expose redacted merge/restore/drop previews and expiring plan IDs.
3. Route authorisation through native user decision; no page or IPC boolean grants consent.

**Tests:**
- Test plan tampering, changed choices, expiry, owner replacement and replayed permits.
- Assert synthetic credential values never appear in previews/debug/IPC.

**Done when:** Sensitive apply can only consume a matching current native decision.

**Evidence:** `docs/testing/versioned-browsing/VB-016.md`.

#### VB-017 — Wire typed versioning inputs/effects through runtime and application
**Dependencies:** VB-011, VB-012, VB-013, VB-016  
**Design sections:** 3, 9.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/reducer.rs`
- `crates/ferric-browser-runtime/src/lib.rs`
- `crates/ferric-browser-runtime/src/ipc_projection.rs`
- `crates/ferric-browser-application/src/lib.rs`
- `crates/ferric-browser-engine-qt/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Implementation:**
1. Add typed reducer/runtime effects and a thin BrowserUi entry/acknowledgement adapter.
2. Keep orchestration in BrowserApplication and stored state behind core reducers.
3. Make unsupported operations return the exact capability reason rather than execute placeholders.

**Tests:**
- Test one command through command/action/runtime/application to a fake effect and back.
- Static boundary regression ensures no new persistence/policy owner is added to Main.qml.

**Done when:** There is one shared execution path ready for vertical feature slices.

**Evidence:** `docs/testing/versioned-browsing/VB-017.md`.

### P2 — Durable structural continuity

#### VB-018 — Introduce schema-1 capture DTOs separate from recovery
**Dependencies:** VB-014, VB-017  
**Design sections:** 10.1, 10.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/sessions.rs`
- `crates/ferric-browser-storage/src/lib.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/mod.rs`
- `crates/ferric-browser-storage/src/versioning/manifest.rs`

**Implementation:**
1. Define bounded explicit manifest DTOs and conversions from safe structural projections.
2. Validate schema, logical IDs, owner profile, finite positions, strings and object-role references.
3. Keep existing SessionSnapshot schema 2 byte/behaviour compatible.

**Tests:**
- Round-trip manifest fixtures; reject malformed/oversized/newer schemas.
- Run existing session recovery tests unchanged and verify pages/cookies are not added to recovery.

**Done when:** Explicit capture and crash recovery have separate formats and admission paths.

**Evidence:** `docs/testing/versioned-browsing/VB-018.md`.

#### VB-019 — Add versioning index migration through the existing profile store
**Dependencies:** VB-018  
**Design sections:** 10  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/lib.rs`
- `crates/ferric-browser-storage/src/worker.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/schema.rs`

**Implementation:**
1. Read the actual migration chain and allocate its next number.
2. Add commit/object/reference/journal/operation tables with foreign-key and uniqueness rules.
3. Keep one profile metadata writer; never open a store for transient owners.

**Tests:**
- Test fresh profile, upgrade, reopen, failed migration rollback and wrong-profile lookup.
- Verify private/ephemeral test owners create no versioning tables or files.

**Done when:** The new schema is reachable only through the normal profile worker and preserves existing data.

**Evidence:** `docs/testing/versioned-browsing/VB-019.md`.

#### VB-020 — Implement restrictive staging paths and object validation
**Dependencies:** VB-018, VB-019  
**Design sections:** 10.3, 20.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/lib.rs`
- `Cargo.toml`
- `Cargo.lock`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/objects.rs`

**Implementation:**
1. Create operation directories with validated containment, restrictive permissions and no title/URL-derived paths.
2. Implement streamed byte limits/hash/size checks with a reviewed hashing dependency or existing reviewed facility.
3. Reject symlinks/path traversal and mismatching existing immutable objects. Dependency additions require authorised review.

**Tests:**
- Test symlink substitution, traversal, oversized files, hash mismatch and interrupted writes.
- Test deterministic object identity and no overwrite of unrelated files.

**Done when:** Untrusted capture data cannot choose or overwrite a filesystem destination.

**Evidence:** `docs/testing/versioned-browsing/VB-020.md`.

#### VB-021 — Add atomic structural commit publication and crash recovery
**Dependencies:** VB-019, VB-020  
**Design sections:** 10.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/worker.rs`
- `crates/ferric-browser-application/src/storage.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/manifest.rs`
- `crates/ferric-browser-storage/src/versioning/journal.rs`

**Implementation:**
1. Extend typed StorageCommand/StorageOperation/StorageCompletion together.
2. Publish validated manifest and references in one metadata transaction after object durability.
3. Record incomplete operations for deterministic orphan quarantine/recovery; serialize same-kind jobs under current correlation rules.

**Tests:**
- Inject failures before/after fsync, rename, SQL transaction and announcement.
- Assert no completed commit can reference unpublished data and tickets cannot cross between requests.

**Done when:** A visible commit is complete at Ferric storage boundaries; interrupted work is not announced as success.

**Evidence:** `docs/testing/versioned-browsing/VB-021.md`.

#### VB-022 — Implement commit and commit-list end to end
**Dependencies:** VB-017, VB-018, VB-021  
**Design sections:** 8, 11.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/lib.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`
- `crates/ferric-browser-core/src/command.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/capture.rs`

**Implementation:**
1. Capture structural tab/window input at the admitted revision and submit through application storage coordination.
2. Return operation ID/progress and announce only a durable completion.
3. Provide bounded current-profile listing and info; pages/editable options remain unavailable until qualified.

**Tests:**
- Exercise command/action/IPC commit on disposable normal profile and list after restart.
- Test later tab mutation does not alter an already admitted immutable capture.
- Verify private and transient automatic persistence refusal.

**Done when:** Users can create and discover real durable structural commits.

**Evidence:** `docs/testing/versioned-browsing/VB-022.md`.

#### VB-023 — Implement safe restore planning and fresh ID allocation
**Dependencies:** VB-018, VB-022  
**Design sections:** 11.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/browser_ui_sessions.rs`
- `crates/ferric-browser-application/src/lib.rs`
- `crates/ferric-browser-core/src/reducer.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Build a restore preview using current safe URL policy and current normal-profile identity.
2. Allocate new runtime IDs with logical lineage mapping and preserve placeholders for unsafe descriptors.
3. Plan selected-first/lazy background navigation into a new compatible window.

**Tests:**
- Test duplicate URLs, invalid stored IDs, missing profile, unsafe POST descriptors and unsupported context references.
- Assert no hidden form submission or history replay is emitted.

**Done when:** Restore is a validated plan, not replay of arbitrary stored events.

**Evidence:** `docs/testing/versioned-browsing/VB-023.md`.

#### VB-024 — Wire safe new-window restore without replacing the source
**Dependencies:** VB-016, VB-023  
**Design sections:** 8, 11.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimePresentation.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Execute new-window restore through current window creation and activation paths.
2. Preserve the source window on allocation/navigation failure and report unsafe pages as placeholders.
3. Keep current-window replacement unavailable until VB-096 qualifies retention and consent; a URL checkpoint is not an unsaved-editor backup.

**Tests:**
- Test failed window creation, stale target and lazy inactive tabs; assert unsupported current-window replacement refuses without mutation.
- Native smoke verifies current window remains intact under default restore.

**Done when:** Default commit/restore opens a usable new normal-profile window without changing the source, page capture or cookies.

**Evidence:** `docs/testing/versioned-browsing/VB-024.md`.

#### VB-025 — Expose structural diff and versioning list/detail UI
**Dependencies:** VB-014, VB-022, VB-024  
**Design sections:** 8, 11.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/action.rs`
- `crates/ferric-browser-core/src/switcher.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`

**Implementation:**
1. Add list/detail/diff projections using one panel and existing switcher integration.
2. Escape titles/messages; paginate differences; display unsupported categories explicitly.
3. Keep saved content out of default query/IPC results.

**Tests:**
- Test keyboard focus/escape/navigation, ambiguous refs and long untrusted labels.
- Test structural diff output against pure planner fixtures and redaction snapshots.

**Done when:** A keyboard-only user can discover, inspect and restore a commit without a graph UI.

**Evidence:** `docs/testing/versioned-browsing/VB-025.md`.

#### VB-026 — Add bounded journal, deletion preview and GC
**Dependencies:** VB-021, VB-022, VB-025  
**Design sections:** 10.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/worker.rs`
- `crates/ferric-browser-application/src/storage.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/retention.rs`

**Implementation:**
1. Record redacted meaningful events with bounded retention.
2. Implement deletion preview and mark/sweep roots including active operations and pinned commits.
3. Quarantine corrupt objects and remove unreferenced staging without touching external files.

**Tests:**
- Test shared objects across commits, active capture racing GC, pinned quota exhaustion and interrupted deletion.
- Verify history clear does not silently erase named commits or claim to delete archives.

**Done when:** Storage growth is bounded without destroying explicitly retained work.

**Evidence:** `docs/testing/versioned-browsing/VB-026.md`.

#### VB-027 — Add allowlisted configuration capture and explicit policy restore
**Dependencies:** VB-014, VB-016, VB-024  
**Design sections:** 11.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-config/src/browser_config.rs`
- `crates/ferric-browser-config/src/settings.rs`
- `crates/ferric-browser-application/src/configuration.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Define an explicit declarative allowlist for versioned configuration values.
2. Diff and preview current versus captured values through existing config validation.
3. Exclude grants, trust exceptions, shell paths and security weakening from automatic replay.

**Tests:**
- Test configuration revision change after preview and obsolete/unknown settings.
- Verify ordinary restore does not change current policy or permissions.

**Done when:** Policy restore is an explicit validated transaction at Ferric level, not whole-config file replacement.

**Evidence:** `docs/testing/versioned-browsing/VB-027.md`.

#### VB-028 — Add normal-profile context scope without broadening identity
**Dependencies:** VB-010, VB-022, VB-024  
**Design sections:** 5.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/profile.rs`
- `crates/ferric-browser-application/src/lib.rs`
- `docs/architecture/ADR-0011-context-profile-window-actions.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/capture.rs`

**Implementation:**
1. Resolve context membership through existing registry and capture exact same-profile owners.
2. Record per-window intervals and invalid members; no global simultaneous snapshot claim.
3. Enable context scope in shared command metadata only after the path works.

**Tests:**
- Test changed membership, closed secondary window, wrong-profile and transient context input.
- Verify no compositor layout or cross-profile data is serialised.

**Done when:** Context capture is a safe aggregation of existing scopes, not a new profile boundary.

**Evidence:** `docs/testing/versioned-browsing/VB-028.md`.

#### VB-029 — Qualify and document structural continuity milestone
**Dependencies:** VB-022, VB-024, VB-025, VB-026, VB-027, VB-028  
**Design sections:** 23, 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `CONTRIBUTING.md`
- `docs/requirements.csv`
- `.github/workflows/ci.yml`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/structural-release.md`

**Implementation:**
1. Run focused core/application/storage tests and the repository full quality gate.
2. Exercise restart, file corruption, quota and normal/private isolation journeys.
3. Publish exact supported scope and outstanding native gates without waiting for cookie work.

**Tests:**
- Existing sessions/contexts/downloads remain regression-clean.
- Real structural commit survives restart and restore does not replay unsafe requests.

**Done when:** Foundation milestone is independently releasable and evidence-backed.

**Evidence:** `docs/testing/versioned-browsing/VB-029.md`.

### P3 — Live stash and retention

#### VB-030 — Add retention leases to pure owner/state model
**Dependencies:** VB-011, VB-017, VB-003  
**Design sections:** 5.2, 12.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/model.rs`
- `crates/ferric-browser-core/src/reducer.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-core/src/versioning/model.rs`

**Implementation:**
1. Represent visible/retained/transferring/closing membership and explicit profile/view leases.
2. Add prepare/acquire/release/lost events with exactly-once ownership accounting.
3. Do not store Qt handles or bypass current profile affinity.

**Tests:**
- Test double release, last-owner close, duplicate view ownership and wrong-profile adoption.
- Assert a valid retention lease prevents a logical close from silently discarding work.

**Done when:** Retention has a testable owner independent of tab-strip visibility.

**Evidence:** `docs/testing/versioned-browsing/VB-030.md`.

#### VB-031 — Introduce the native retained-view host
**Dependencies:** VB-030, VB-003  
**Design sections:** 12.2  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeServices.qml`
- `crates/ferric-browser-engine-qt/build.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningViewHost.qml`

**Implementation:**
1. Implement actual view ownership under a live profile-compatible retaining component.
2. Change view-pool reconciliation to distinguish retained from stale/unmatched views.
3. Retain exact handles under opaque IDs; do not create replacements on a missing handle.

**Tests:**
- Native test hides/removes a tab from presentation without destroying its sentinel view.
- Test no retained view is included twice in ordinary tab counts or leaked on explicit release.

**Done when:** The same WebEngineView remains owned when absent from the working tab strip.

**Evidence:** `docs/testing/versioned-browsing/VB-031.md`.

#### VB-032 — Create explicit-stash eligibility and dirty-work consent
**Dependencies:** VB-016, VB-030  
**Design sections:** 12.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/browser_ui_tabs.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimePresentation.qml`
- `docs/testing/m0-96-tab-suspension.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/stash.rs`

**Implementation:**
1. Keep existing suspension/discard policy unchanged and introduce separate explicit preservation intent.
2. Check capture/audio/DevTools/navigation/prompt conditions and dirty editable state.
3. Allow qualified dirty retention only after native disclosure that data remains volatile; otherwise refuse clearly.

**Tests:**
- Test ordinary suspend still refuses dirty/unknown editors.
- Test stash never silently hides a live call, capture prompt or unsupported view.

**Done when:** The unsaved-work use case is enabled through deliberate preservation, not relaxed discard guards.

**Evidence:** `docs/testing/versioned-browsing/VB-032.md`.

#### VB-033 — Implement single-tab stash preparation and freeze acknowledgement
**Dependencies:** VB-031, VB-032, VB-017  
**Design sections:** 9.3, 12  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningViewHost.qml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/stash.rs`

**Implementation:**
1. Preallocate fallback tab/host, acquire leases and hide the original only through the admitted operation.
2. Request Frozen and verify the actual lifecycle acknowledgement before announcing a frozen stash.
3. Roll back prepared membership/leases on refusal or stale target; preserve the source.

**Tests:**
- Test fallback allocation failure, freeze refusal, duplicate/late acknowledgement and target navigation.
- Native fixture confirms original identity and no reload.

**Done when:** Single-tab stash works end to end and cannot fake a successful freeze.

**Evidence:** `docs/testing/versioned-browsing/VB-033.md`.

#### VB-034 — Implement stash resume with prepare/adopt/consume
**Dependencies:** VB-033  
**Design sections:** 12.3  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/stash.rs`

**Implementation:**
1. Resolve exact lease and original compatible host, prepare target membership and activate/adopt the same view.
2. Consume stash ownership only after successful adoption.
3. On failure return to retained state; do not destroy the page or duplicate the stash entry.

**Tests:**
- Test missing renderer, wrong profile, failed destination adoption and stale owner.
- Verify a resumed and modified page is not claimed to remain an immutable stash.

**Done when:** Resume continues the actual instance and consumes one retention lease exactly once.

**Evidence:** `docs/testing/versioned-browsing/VB-034.md`.

#### VB-035 — Implement window stash as a group operation
**Dependencies:** VB-033, VB-034  
**Design sections:** 9.3, 12  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/reducer.rs`
- `crates/ferric-browser-application/src/versioning/stash.rs`

**Implementation:**
1. Prepare all members and replacement visibility before committing group membership.
2. Rollback prepared members when any mandatory member cannot be retained.
3. Record later per-member renderer loss without rewriting the original group result.

**Tests:**
- Inject failure at each member index and assert original window remains usable.
- Test maximum scope, selected-tab restoration and no partial silent stash.

**Done when:** Window stashes have explicit all-or-failed admission and per-page later fidelity.

**Evidence:** `docs/testing/versioned-browsing/VB-035.md`.

#### VB-036 — Extend profile owner accounting for hidden stashes
**Dependencies:** VB-031, VB-035  
**Design sections:** 12.2, 12.4  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`

**Implementation:**
1. Count hidden view leases and pending native operations alongside visible window owners.
2. Prevent final profile release while a retained view exists.
3. Confirm user intent on visible host close; keep owner host or use the qualified same-profile transfer path.

**Tests:**
- Test final visible window closing with stashes, secondary/primary ownership and explicit final release.
- Assert views are destroyed before profile and no alternate profile is selected by fallback.

**Done when:** Window close no longer silently destroys retained pages or prematurely clears branch profiles.

**Evidence:** `docs/testing/versioned-browsing/VB-036.md`.

#### VB-037 — Report renderer loss and prohibit false live restoration
**Dependencies:** VB-034, VB-036  
**Design sections:** 12.4  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.live_retention`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeChrome.qml`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/stash.rs`

**Implementation:**
1. Route per-view renderer termination into exact retention loss events.
2. Expose safe independent fallback references without loading them automatically.
3. Make resume of a lost stash return RetainedRendererLost with an explicit fallback choice.

**Tests:**
- Crash a disposable renderer while two views/stashes may share a process.
- Verify no phantom LIVE status, unbounded retries or automatic reload masking loss.

**Done when:** Loss is observable and restoration never upgrades a weaker fallback to LIVE.

**Evidence:** `docs/testing/versioned-browsing/VB-037.md`.

#### VB-038 — Add stash list, drop preview and resource visibility
**Dependencies:** VB-025, VB-034, VB-037  
**Design sections:** 8, 21  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`
- `crates/ferric-browser-core/src/action.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/projection.rs`

**Implementation:**
1. Display retained view/profile counts, actual lifecycle, volatility and labelled memory attribution.
2. Add explicit drop confirmation when the stash is the only live copy.
3. Enforce count limits at admission without automatic eviction.

**Tests:**
- Test limits, shared renderer memory accounting, escape/cancel and hostile labels.
- Confirm drop releases native resources only after the matching user decision.

**Done when:** Users can understand and control the cost of retained state.

**Evidence:** `docs/testing/versioned-browsing/VB-038.md`.

#### VB-039 — Integrate stash shutdown and cancellation
**Dependencies:** VB-036, VB-037, VB-038  
**Design sections:** 12.4, 22  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeBase.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`
- `crates/ferric-browser-application/src/lib.rs`

**Implementation:**
1. Add volatile-loss summary to the existing shutdown decision path.
2. Cancel outstanding retention operations and prevent new admissions during shutdown.
3. Destroy views before profiles and ignore late callbacks without persisting live state.

**Tests:**
- Test normal quit, cancellation of quit, force path, pending callback and OS-kill outcome documentation.
- Verify no cookie/page payload is written as a hidden live recovery mechanism.

**Done when:** Shutdown semantics are explicit and consistent with ephemeral final-owner rules.

**Evidence:** `docs/testing/versioned-browsing/VB-039.md`.

#### VB-040 — Qualify same-profile cross-window stash restoration
**Dependencies:** VB-034, VB-036, VB-006  
**Design sections:** 12.2  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/testing/m0-157-tab-transfer-contract.md`
- `docs/testing/m0-160-secondary-tab-transfer.md`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_reparent.qml`

**Implementation:**
1. Exercise primary/secondary live restore through existing transfer contracts.
2. Preflight replacement source slots and support failure rollback without a reload.
3. Keep unsupported compositor/reparent configurations unavailable rather than emulating with URL reload.

**Tests:**
- Run native Wayland focus/reparent at fractional scale and source-close cases.
- Validate preserved instance sentinel and no profile reassignment.

**Done when:** Cross-window LIVE claims have actual platform evidence.

**Evidence:** `docs/testing/versioned-browsing/VB-040.md`.

#### VB-041 — Publish live continuity capability certificate
**Dependencies:** VB-033, VB-034, VB-035, VB-036, VB-037, VB-038, VB-039, VB-040  
**Design sections:** 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `CONTRIBUTING.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/live-release.md`

**Implementation:**
1. Run the representative lifecycle/dirty-editor/resource fixtures and adapter/Wayland gates.
2. Record supported/refused activity categories and exact engine/runtime fingerprint.
3. Enable live-retention only for the qualified path and preserve negative capability tests.

**Tests:**
- Repeated stash/resume/drop has no unbounded view/profile leak.
- Dirty work, renderer loss and shutdown messages match actual behaviour.

**Done when:** LIVE means retained instance on a documented runtime, not immutable execution snapshot.

**Evidence:** `docs/testing/versioned-browsing/VB-041.md`.

### P4 — Page capture, archives and reconstruction

#### VB-042 — Add explicit sensitive capture consent and page policies
**Dependencies:** VB-001, VB-016, VB-022, VB-025  
**Design sections:** 13, 20  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-config/src/settings.rs`
- `crates/ferric-browser-application/src/versioning/capture.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`

**Implementation:**
1. Implement page capture ask/deny settings and explicit editable opt-in.
2. Explain full archive sensitivity and ordinary local-file protection; no sanitisation claim.
3. Keep private capture persistence and automatic checkpoint page collection disabled.

**Tests:**
- Test cancelled consent, per-operation scope change and private/ephemeral origin.
- Verify safe metadata redaction does not cause the UI to call an archive secret-free.

**Done when:** Archive capture requires an informed current native request.

**Evidence:** `docs/testing/versioned-browsing/VB-042.md`.

#### VB-043 — Add bounded capture job staging and blob I/O scheduling
**Dependencies:** VB-020, VB-021, VB-042  
**Design sections:** 10.3, 13.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/worker.rs`
- `crates/ferric-browser-application/src/storage.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-storage/src/versioning/objects.rs`
- `crates/ferric-browser-application/src/versioning/capture.rs`

**Implementation:**
1. Reserve disk budget and create exact staging destinations before invoking native save.
2. Use bounded blob I/O work without a second metadata database writer.
3. Carry StorageTicket/operation correlation through validation and publication.

**Tests:**
- Test queue saturation, same-kind correlation, cancellation during hash and disk quota reservation.
- Assert metadata queries remain responsive during a bounded large blob job.

**Done when:** Page-size I/O cannot block UI or corrupt unrelated storage completion routing.

**Evidence:** `docs/testing/versioned-browsing/VB-043.md`.

#### VB-044 — Route native page-save downloads under exact permits
**Dependencies:** VB-043, VB-004  
**Design sections:** 13.2  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.archive_capture`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeChrome.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Implementation:**
1. Invoke QML save only after the permit/path is installed for the captured profile/view.
2. Correlate public save download fields with the expected job; fail on ambiguity.
3. Handle auto-start semantics without globally accepting unrelated downloads or using private page pointers.

**Tests:**
- Run capture alongside ordinary downloads and page-triggered extra downloads.
- Test stale permit, wrong profile, missing public match and auto-start cancellation.
- Keep ordinary viewless download-to-ledger qualification separate under VB-097; the exact page-save permit is not a general download fallback.

**Done when:** Only the preauthorised page-save output can become a capture object.

**Evidence:** `docs/testing/versioned-browsing/VB-044.md`.

#### VB-045 — Complete archive save validation and partial coverage
**Dependencies:** VB-044, VB-021  
**Design sections:** 13.2, 13.3  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.archive_capture`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/capture.rs`
- `crates/ferric-browser-storage/src/versioning/manifest.rs`

**Implementation:**
1. Distinguish completed/cancelled/interrupted/empty/native-lost states.
2. After asynchronous sidecar collection, revalidate that the exact captured view still owns its logical tab and still shows the captured URL before accepting sidecar output or claiming Qt's auto-starting save request; report document change or native-view loss separately, including when the native save call throws and when the view is absent at archive completion.
3. Validate object bytes and attach a capture-interval/coverage result to the correct logical page.
4. Publish a partial commit only when operation policy permits it; otherwise keep no visible commit.

**Tests:**
- Inject document navigation, timeout, disk full and multi-tab one-page failure.
- Verify isFinished without DownloadCompleted never becomes archive success.

**Done when:** A saved archive has verified bytes and truthful per-page coverage.

**Evidence:** `docs/testing/versioned-browsing/VB-045.md`.

#### VB-046 — Define bounded page-sidecar DTOs and validation
**Dependencies:** VB-018, VB-009  
**Design sections:** 14.1, 14.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/versioning/capture.rs`
- `crates/ferric-browser-storage/src/versioning/manifest.rs`

**Implementation:**
1. Define frames/candidates/positions/category coverage/helper version with explicit limits.
2. Represent sensitive editable values separately from safe projections.
3. Reject invalid origins, non-finite coordinates, duplicate candidate identity and oversized nested data.

**Tests:**
- Fuzz or table-test untrusted sidecar parsing, unknown schemas and extreme values.
- Verify Debug/UI/IPC projections cannot include editable values.

**Done when:** A sidecar is bounded untrusted data, never native authority.

**Evidence:** `docs/testing/versioned-browsing/VB-046.md`.

#### VB-047 — Implement top-document geometry collector
**Dependencies:** VB-046, VB-006  
**Design sections:** 14.1, 14.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js`
- `crates/ferric-browser-engine-qt/build.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/qml/scripts/VersionedPageState.js`

**Implementation:**
1. Add reviewed ApplicationWorld collector for top/nested eligible scroll and simple presentation categories.
2. Use bounded traversal, elapsed-time checks and a closed result schema.
3. Do not capture the ADR-0014 explicit scroll-target object or persist its weak reference/selector.

**Tests:**
- Test long/mutating DOM, detached scrollers, non-finite properties and traversal cap.
- Verify current scroll-target policy remains document-local and unchanged by capture.

**Done when:** Geometry capture works within declared limits without retaining arbitrary DOM trees.

**Evidence:** `docs/testing/versioned-browsing/VB-047.md`.

#### VB-048 — Add opt-in editable and selection capture
**Dependencies:** VB-042, VB-046, VB-047  
**Design sections:** 14.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/scripts/VersionedPageState.js`

**Implementation:**
1. Capture only permitted visible control types under explicit editable consent.
2. Exclude password/file/hidden token/OTP-sensitive fields; report rather than hide exclusions.
3. Keep contenteditable as a bounded recoverable text draft and selection optional.

**Tests:**
- Use synthetic secret fields and verify excluded values never reach sidecar/logs.
- Test large values, ambiguous controls, editable shadow content and partial coverage.

**Done when:** Useful drafts can be preserved without claiming universal secret sanitisation.

**Evidence:** `docs/testing/versioned-browsing/VB-048.md`.

#### VB-049 — Add bounded per-frame capture with origin policy
**Dependencies:** VB-047, VB-048  
**Design sections:** 14.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeServices.qml`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`

**Implementation:**
1. Use public mainFrame/children/isValid APIs where available, starting with same-origin permitted scope.
2. Correlate each asynchronous result to frame/document/operation identity.
3. Report cross-origin-unapproved/opaque/deleted frames explicitly; never use child-array order as persistent identity.

**Tests:**
- Test frame removal/reordering/navigation, maximum depth/count and denied embedded origins.
- Verify no metadata or authority leaks through a frame result into native commands.

**Done when:** Frame coverage is real and bounded, not inferred from top-document success.

**Evidence:** `docs/testing/versioned-browsing/VB-049.md`.

#### VB-050 — Integrate sidecar/archive capture intervals
**Dependencies:** VB-045, VB-046, VB-049  
**Design sections:** 13.3, 14  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/capture.rs`
- `crates/ferric-browser-storage/src/versioning/manifest.rs`

**Implementation:**
1. Collect sidecar and archive results under one owner/document token with capture start/end facts.
2. Handle DOM mutation evidence as a coverage/interval warning, not an atomicity certificate.
3. Publish helper/backend versions and category-level completeness in the manifest.

**Tests:**
- Race navigation and same-document mutation between sidecar and archive completion.
- Test failed collector with successful archive and the inverse; retain correct independent fidelity dimensions.

**Done when:** Mixed-document or falsely atomic page commits cannot be published.

**Evidence:** `docs/testing/versioned-browsing/VB-050.md`.

#### VB-051 — Create restricted archive profile and load path
**Dependencies:** VB-004, VB-045, VB-016  
**Design sections:** 15  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.archive_offline`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/request_interceptor.h`
- `crates/ferric-browser-engine-qt/src/request_interceptor.cpp`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeBase.qml`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningArchiveView.qml`

**Implementation:**
1. Create a distinct OTR archive profile with scripts/permissions/native bridges disabled before load.
2. Attach deny-by-default network/local-file policy while allowing only the authorised archive representation.
3. Separate untrusted archive presentation from privileged Ferric internal UI.

**Tests:**
- Observe attempted CSS/image/frame/redirect traffic and blocked form/pop-up/download/protocol actions.
- Verify using an archive never attaches the normal profile or replaces its interceptor.

**Done when:** Historical viewing is enabled only when the offline isolation boundary is demonstrated.

**Evidence:** `docs/testing/versioned-browsing/VB-051.md`.

#### VB-052 — Expose archive viewing and explicit open-live transition
**Dependencies:** VB-025, VB-051  
**Design sections:** 15.2  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.archive_offline`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Add archive mode to restore/info with capture date/source/fidelity labels.
2. Implement Open live as a distinct authorised normal navigation target.
3. Keep corrupt/missing archives unavailable rather than substituting current URLs.

**Tests:**
- Test keyboard accessibility, corrupt object, source metadata escaping and open-live confirmation.
- Verify archive and live targets have visibly different state and profile handling.

**Done when:** Users can inspect historical content without accidentally reconnecting the archive.

**Evidence:** `docs/testing/versioned-browsing/VB-052.md`.

#### VB-053 — Implement conservative candidate matching and geometry restore
**Dependencies:** VB-023, VB-046, VB-047  
**Design sections:** 14.2, 14.5  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/restore.rs`
- `crates/ferric-browser-engine-qt/qml/scripts/VersionedPageState.js`

**Implementation:**
1. Load safe current URLs and validate route/origin/document after redirects.
2. Match unique bounded candidates by descriptors; skip ambiguous or changed targets.
3. Restore geometry in a bounded readiness window and leave explicit keyboard scroll policy at auto.

**Tests:**
- Test changed layouts, duplicate IDs, route changes, nested scrollers and timeout.
- Assert no stored selector is treated as authority across documents.

**Done when:** Geometry restore is useful but explicitly partial where the live page changed.

**Evidence:** `docs/testing/versioned-browsing/VB-053.md`.

#### VB-054 — Implement editable draft recovery and guarded refill
**Dependencies:** VB-048, VB-053, VB-016  
**Design sections:** 14.5  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/scripts/VersionedPageState.js`
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`

**Implementation:**
1. Show recovered drafts without injecting arbitrary captured HTML.
2. Permit confirmed unique native-control property refill only; no synthetic submit/input/change events.
3. Verify results once and report app-overwritten/refused values rather than retry indefinitely.

**Tests:**
- Test controlled SPA inputs, blur autosave, wrong account/route and rejected matches.
- Verify no automatic click, submission, focus change or contenteditable HTML execution.

**Done when:** Failed reconstruction gives a local recoverable draft instead of invasive website automation.

**Evidence:** `docs/testing/versioned-browsing/VB-054.md`.

#### VB-055 — Add media metadata without automatic playback
**Dependencies:** VB-046, VB-053  
**Design sections:** 14.5  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/scripts/VersionedPageState.js`
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Capture bounded media position/paused metadata where permitted.
2. Present optional restore position under existing media policy; never restart capture/calls/playback.
3. Label streaming/DRM/live content position unsupported when it cannot be reconstructed.

**Tests:**
- Test media element replacement, invalid duration/time, playing versus paused and live streams.
- Assert no autoplay, microphone, camera or screen-capture action occurs.

**Done when:** Media state adds convenience without implying preserved media execution sessions.

**Evidence:** `docs/testing/versioned-browsing/VB-055.md`.

#### VB-056 — Add capture cleanup, corruption and privacy fault tests
**Dependencies:** VB-026, VB-043, VB-045, VB-050, VB-051  
**Design sections:** 10.4, 20, 22  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-storage/src/versioning/retention.rs`
- `crates/ferric-browser-application/src/versioning/capture.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_capture_faults.qml`

**Implementation:**
1. Exercise abandoned staging, cancelled native downloads and profile teardown while hashing.
2. Audit logging/IPC/exception paths for synthetic secrets and archive path leakage.
3. Verify quota overshoot is reported and cleaned, with no false exact hard-cap claim.

**Tests:**
- Run restart/corruption/symlink and late-callback fixtures.
- Verify private/branch contents cannot appear through automatic normal-profile capture.

**Done when:** Capture failure paths are as explicit and private as successful publication.

**Evidence:** `docs/testing/versioned-browsing/VB-056.md`.

#### VB-057 — Qualify page continuity and archive release surface
**Dependencies:** VB-050, VB-052, VB-053, VB-054, VB-055, VB-056  
**Design sections:** 23, 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `CONTRIBUTING.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/page-release.md`

**Implementation:**
1. Run representative document/editor/frame/media/archive cases on the declared engine.
2. Publish exact capture and restore category coverage, including common unavailable cases.
3. Enable each archive/semantic capability independently; avoid all-or-nothing false confidence.

**Tests:**
- Native archive zero-traffic and safe reconstruction cases pass.
- A controlled app that refuses reconstruction is correctly reported partial, not forced.

**Done when:** Page continuity is useful, qualified and honest about reconstructive limits.

**Evidence:** `docs/testing/versioned-browsing/VB-057.md`.

### P5 — Clean isolated branches

#### VB-058 — Create branch profile leases without normal-store inheritance
**Dependencies:** VB-011, VB-017, VB-001  
**Design sections:** 16.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeBase.qml`
- `crates/ferric-browser-application/src/profile_open.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`

**Implementation:**
1. Allocate a distinct Quick profile instance and separate owner token per clean branch.
2. Apply current security policy and memory-only storage configuration before navigation.
3. Track branch view/profile leases without opening the parent ProfileStore or durable context.

**Tests:**
- Test two clean branches have independent cookies/storage and do not share defaultProfile.
- Verify memory-only roots, allocation failure and final-owner destruction order.

**Done when:** A clean branch is an actual local storage/profile boundary.

**Evidence:** `docs/testing/versioned-browsing/VB-058.md`.

#### VB-059 — Implement visible-parent single-tab clean branch transaction
**Dependencies:** VB-058, VB-014, VB-016  
**Design sections:** 16.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-engine-qt/src/browser_ui_versioning.rs`
- `crates/ferric-browser-core/src/versioning/model.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`

**Implementation:**
1. Prepare child host/profile while keeping the parent visible and active.
2. Capture structural source, establish ancestry and navigate a safe child URL without moving, hiding, freezing or closing the parent view.
3. Publish explicit parent-active and child initial coverage; fail without damaging source. Hidden-parent retention is separately gated by VB-098.

**Tests:**
- Test source POST/unsafe URL, child allocation failure and failed child navigation while the parent remains usable.
- Verify child runtime IDs differ and profile never changes on the existing parent view.

**Done when:** branch NAME --clean creates inspectable isolated divergence while its original parent view stays live and usable.

**Evidence:** `docs/testing/versioned-browsing/VB-059.md`.

#### VB-060 — Wire cross-owner line registry and exact branch switching
**Dependencies:** VB-059, VB-017  
**Design sections:** 5.3, 16.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/lib.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`
- `crates/ferric-browser-core/src/action.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`

**Implementation:**
1. Route branch IDs through an application-level coordinator to exact owner/profile instances.
2. Use existing native focus/activation paths; never fall back to the primary owner when a branch target is stale.
3. Switch between still-live parent and child owners without reloading; hidden-parent resume is reserved for VB-098.

**Tests:**
- Test identical local runtime TabIds in different owner instances, ambiguous names and closed host.
- Native smoke switches parent/child windows without profile reassignment.

**Done when:** switch selects a real line of work, not whichever window happens to be active.

**Evidence:** `docs/testing/versioned-browsing/VB-060.md`.

#### VB-061 — Add visible-parent window-scope clean branches
**Dependencies:** VB-059  
**Design sections:** 5.4, 16.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-core/src/versioning/model.rs`

**Implementation:**
1. Create one child profile for all selected same-profile source-window tabs.
2. Preserve logical ordering and selected page while leaving the parent window visible and active; hidden group retention remains VB-098.
3. Record per-page child load failures without claiming globally simultaneous state.

**Tests:**
- Test large bounded scope, mixed-profile rejection, background lazy loads and one failed child URL.
- Verify every child popup/tab remains in the one branch profile.

**Done when:** Window branching is a coherent group operation rather than independent unrelated duplicates.

**Evidence:** `docs/testing/versioned-browsing/VB-061.md`.

#### VB-062 — Add page-enriched clean branch reconstruction
**Dependencies:** VB-059, VB-053, VB-054, VB-050  
**Design sections:** 16.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Reuse the capture/reconstruction pipeline for approved source sidecar categories.
2. Capture before any optional hidden-parent retention; the visible-parent mode never freezes a collector.
3. Display reconstruction coverage independently of parent live retention and identity mode.

**Tests:**
- Test clean profile redirects to login, route/account mismatch and controlled-input overwrite.
- Assert original page instance is preserved while child has no cloned JS heap.

**Done when:** Branching can inherit useful UI state without overstating runtime cloning.

**Evidence:** `docs/testing/versioned-browsing/VB-062.md`.

#### VB-063 — Keep branch popups and permission decisions inside the branch
**Dependencies:** VB-058, VB-059  
**Design sections:** 16.1, 20  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricPopupWindow.qml`
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml`

**Implementation:**
1. Bind new-window/opener flows to the originating branch profile and owner.
2. Keep permission and certificate prompts fresh/current rather than copying normal grants.
3. Reject cross-profile adoption and hidden normal-profile fallbacks.

**Tests:**
- Test target=_blank, scripted popup policy, OAuth-style local fixture redirects and permission prompts.
- Verify branch can neither inherit saved trust exceptions nor send its popup to normal profile accidentally.

**Done when:** All secondary browsing from a branch preserves the declared local isolation.

**Evidence:** `docs/testing/versioned-browsing/VB-063.md`.

#### VB-064 — Implement visible-parent branch discard and resource cleanup
**Dependencies:** VB-060, VB-061, VB-063  
**Design sections:** 20, 21  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`

**Implementation:**
1. Preview only-live losses and durable references before deletion.
2. Cancel jobs, destroy branch views, release secret handles and finally destroy profile.
3. Keep explicitly pinned/exported commits intact and prevent deleting a parent with unresolved ownership references.

**Tests:**
- Test nested branch deletion, active child operation and repeated disposal while the visible parent remains intact; hidden stash-lease cleanup remains VB-098.
- Measure eventual release without claiming instant RSS return from Chromium.

**Done when:** Discarding a branch needs no cookie rollback against the parent and leaves no live owner leaks.

**Evidence:** `docs/testing/versioned-browsing/VB-064.md`.

#### VB-065 — Add branch inspection and contextual safety labels
**Dependencies:** VB-025, VB-060, VB-064  
**Design sections:** 6, 8, 16.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`
- `crates/ferric-browser-core/src/switcher.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/projection.rs`

**Implementation:**
1. Show lineage, visible parent's active status (or separately qualified retention), actual profile mode, page coverage and volatility.
2. Display remote-action warning during creation and avoid untouched/fully-cloned claims.
3. Expose snapshot-mode unavailable reason and an explicit clean-branch alternative.

**Tests:**
- Keyboard-only branch/list/switch/delete journey; untrusted labels and ambiguous refs.
- Assert unavailable snapshot mode cannot silently run shared or clean.

**Done when:** Power users can inspect exactly what their branch isolates and preserves.

**Evidence:** `docs/testing/versioned-browsing/VB-065.md`.

#### VB-066 — Add explicit structural export from linked transient branch
**Dependencies:** VB-001, VB-022, VB-059, VB-016  
**Design sections:** 16.5, 20.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-application/src/versioning/capture.rs`
- `crates/ferric-browser-application/src/profile_open.rs`

**Implementation:**
1. Implement the narrow user-confirmed branch-to-normal-owner export authorised by the ADR.
2. Use safe structural DTOs and a new durable commit identity; child history remains transient.
3. Reject unlinked private owners and do not copy profile databases/cookies.

**Tests:**
- Test cancelled export, wrong destination, owner closure and private-window attempts.
- Restart confirms exported structure exists but no live authenticated branch is implied.

**Done when:** A user may deliberately keep branch work without weakening automatic ephemeral privacy.

**Evidence:** `docs/testing/versioned-browsing/VB-066.md`.

#### VB-067 — Add explicit page export from linked transient branch
**Dependencies:** VB-066, VB-042, VB-050, VB-056  
**Design sections:** 16.5, 20.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/capture.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`

**Implementation:**
1. Require separate page/editable sensitivity consent for a transient branch export.
2. Publish archive/sidecar objects through normal controlled staging, never child automatic recovery.
3. Describe exported capture versus volatile profile identity distinctly.

**Tests:**
- Test cancellation after native save, private source refusal and lost parent store.
- Verify raw cookies and branch mutation journal never enter exported objects.

**Done when:** Durable child page captures exist only after a clear intentional export.

**Evidence:** `docs/testing/versioned-browsing/VB-067.md`.

#### VB-068 — Handle branch restart and independent continued use
**Dependencies:** VB-023, VB-060, VB-064, VB-066  
**Design sections:** 5.3, 16.5  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-application/src/versioning/restore.rs`

**Implementation:**
1. Keep transient lines absent after restart unless a deliberate durable export exists.
2. Reopen exported work as a fresh clean or separately qualified inherited line, never historical live identity.
3. Allow continued child use without requiring merge; do not force promotion into normal profile.

**Tests:**
- Test restart after exported and unexported branches, parent changes and missing archive.
- Verify no current parent-cookie fallback is labelled historical child identity.

**Done when:** Branch lifetime and durable captured history have separate predictable meanings.

**Evidence:** `docs/testing/versioned-browsing/VB-068.md`.

#### VB-069 — Qualify clean exploration milestone
**Dependencies:** VB-060, VB-061, VB-062, VB-063, VB-064, VB-065, VB-066, VB-067, VB-068  
**Design sections:** 23, 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `CONTRIBUTING.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/branch-release.md`

**Implementation:**
1. Run parent-failure, child-failure, nested ownership, popup and privacy journeys.
2. Test resources and native switch/focus under the declared compositor.
3. Publish clean-branch capability without waiting for cookie seeding.

**Tests:**
- Parent remains usable through failed child operations.
- Branch effects do not mutate parent local storage/cookies, while remote-state warning remains explicit.

**Done when:** The flagship isolated exploration workflow ships independently of conditional identity features.

**Evidence:** `docs/testing/versioned-browsing/VB-069.md`.

### P6 — Cookie observation and conditional snapshot inheritance

#### VB-070 — Implement the cookie backend contract and secret projections
**Dependencies:** VB-015, VB-002, VB-017  
**Design sections:** 17.1, 17.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/lib.rs`
- `crates/ferric-browser-engine-qt/src/lib.rs`
- `crates/ferric-browser-core/src/versioning/merge.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`

**Implementation:**
1. Add backend capability/result types with exact scope/fidelity and observation-vs-snapshot distinction.
2. Keep values in bounded memory-only containers with redacted Debug/projection handling.
3. Default general Qt profile snapshot/read-back guarantees to unavailable/observed-only.

**Tests:**
- Test lossy identity, unknown partition, missing read boundary and mismatched engine fingerprint.
- Assert no API accepts a diagnostic mirror as a complete snapshot.

**Done when:** The type contract prevents unsupported backend guarantees from being assumed upstream.

**Evidence:** `docs/testing/versioned-browsing/VB-070.md`.

#### VB-071 — Add the narrow public Quick cookie observation adapter
**Dependencies:** VB-070, VB-005  
**Design sections:** 7, 17.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/build.rs`
- `crates/ferric-browser-engine-qt/src/lib.rs`
- `crates/ferric-browser-engine-qt/src/request_interceptor.h`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.h`
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`

**Implementation:**
1. Use QQuickWebEngineProfile::cookieStore with lifetime-safe QObject references.
2. Forward minimal cookieAdded/Removed observations with profile epoch and bounded queue; no heavy work in IO callbacks.
3. Expose explicit enumeration request without inventing completion/callback signatures absent from the header.

**Tests:**
- Compile against real public headers and test profile destruction during queued observations.
- Overflow invalidates observation state; no raw values reach logs or ordinary QML models.

**Done when:** A public-API adapter can observe synthetic cookies without weakening engine ownership.

**Evidence:** `docs/testing/versioned-browsing/VB-071.md`.

#### VB-072 — Implement memory-only cookie mirror with invalidation
**Dependencies:** VB-071, VB-070  
**Design sections:** 17.3, 17.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`

**Implementation:**
1. Maintain observation epoch/sequence and bounded current records.
2. Separate rescan observations from mutation intent where knowable; otherwise mark observed-only.
3. Invalidate on overflow, collision, malformed identity, profile replacement or unsupported attributes.

**Tests:**
- Test duplicate enumeration, removal/add replacement, reordering and empty timeout.
- Assert incompleteness never manufactures deletion tombstones or exact T0.

**Done when:** The mirror is diagnostically useful without claiming completeness it cannot prove.

**Evidence:** `docs/testing/versioned-browsing/VB-072.md`.

#### VB-073 — Qualify cookie conversion and modern identity fidelity
**Dependencies:** VB-071, VB-072, VB-006  
**Design sections:** 17.2, 17.5  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`
- `docs/testing/versioned-browsing/cookie-api-gap.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_cookie_identity.qml`

**Implementation:**
1. Round-trip supported host-only/domain/path/session/Secure/HttpOnly/SameSite records in fixtures.
2. Probe same-name partitioned variants and source attributes actually exposed by the qualified engine.
3. Document lost/unknown dimensions and keep full-profile strict capability false if identity cannot be preserved.

**Tests:**
- Verify host-only conversion does not broaden to subdomains.
- Identity collisions must fail, not be overwritten or filtered using unavailable metadata.

**Done when:** Exact supported descriptor scope is evidenced; generic Chromium cookie losslessness is never inferred.

**Evidence:** `docs/testing/versioned-browsing/VB-073.md`.

#### VB-074 — Establish or reject a real source snapshot boundary
**Dependencies:** VB-072, VB-073, VB-005  
**Design sections:** 2.3, 17.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/cookie-snapshot-boundary.md`

**Implementation:**
1. Investigate only approved public mechanisms for complete consistent reads on the requested source scope.
2. Stress initial persistent enumeration, in-flight Set-Cookie, JS mutations and expiration.
3. If no boundary exists, record unsupported and add tests rejecting timeout/sentinel-based false completion; do not add private APIs or CDP without separate approval.

**Tests:**
- Reference fixture truth is compared with snapshots under concurrent activity where an actual snapshot mechanism exists.
- Negative path proves strict snapshot callers cannot proceed on observed-only data.

**Done when:** A real scoped snapshot capability is proven or its unavailability is an explicit maintained result.

**Evidence:** `docs/testing/versioned-browsing/VB-074.md`.

#### VB-075 — Implement destination seed and read-back contract for supported fixtures
**Dependencies:** VB-070, VB-073  
**Design sections:** 16.3, 17  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`

**Implementation:**
1. Build a seed plan for complete synthetic/supported snapshots only and keep destination un-navigated.
2. Apply supported records with correct origin/attributes and correlate observations without treating arbitrary notifications as transaction completion.
3. Require reliable destination read-back or return CookieSeedUnverified; destroy a failed prepared child.

**Tests:**
- Test missing acknowledgement, rejected/expired cookies, attribute change and cancellation.
- Fixture results must not enable arbitrary-source snapshot mode by themselves.

**Done when:** Seeding has explicit successful, failed and unverified outcomes with no silent partial child.

**Evidence:** `docs/testing/versioned-browsing/VB-075.md`.

#### VB-076 — Wire snapshot-isolated branch creation behind capabilities
**Dependencies:** VB-059, VB-074, VB-075  
**Design sections:** 16.3  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.cookie_identity`, `versioning.cookie_snapshot`, `versioning.cookie_seed_verify`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-application/src/versioning/cookies.rs`

**Implementation:**
1. Require qualified identity/source snapshot/destination verification before child navigation.
2. Capture full requested parent scope with explicit credential-scope consent, seed the isolated child, then proceed through existing branch transaction.
3. On any unsupported gate return a specific failure and leave the parent intact.

**Tests:**
- Test parent updates after snapshot, child-only writes and child deletion.
- Verify source jar is never a live fallback and requested snapshot never silently becomes clean/shared.

**Done when:** When available, seeded branches provide only the exact inheritance guarantee supported by their backend.

**Evidence:** `docs/testing/versioned-browsing/VB-076.md`.

#### VB-077 — Add cookie delta/expiry reporting and branch diff
**Dependencies:** VB-072, VB-015, VB-065  
**Design sections:** 17.3, 18.1  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`
- `crates/ferric-browser-application/src/versioning/projection.rs`

**Implementation:**
1. Compute deltas only against a valid complete base; otherwise show observation-only metadata.
2. Report additions/changes/deletions/expiry with opaque value handles and coverage flags.
3. Keep cookie material out of durable journal and normal IPC.

**Tests:**
- Test expired bases, incomplete map, rescan duplicates and synthetic secrets.
- Verify unknown absence never appears as branch deletion in mergeable output.

**Done when:** Cookie diff is useful without giving observation data stronger semantics.

**Evidence:** `docs/testing/versioned-browsing/VB-077.md`.

#### VB-078 — Test synthetic authentication inheritance and remote coupling
**Dependencies:** VB-076, VB-006, VB-077  
**Design sections:** 16.4, 17.5  
**Type:** qualification; **initial status:** not-started

**Production capability gates:** `versioning.cookie_snapshot`, `versioning.cookie_seed_verify`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/testing/versioned-browsing/README.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_seeded_identity.qml`

**Implementation:**
1. Use a local multi-origin login/CSRF fixture under the qualified backend.
2. Verify local parent/child divergence and demonstrate server revocation remains shared/external.
3. Record apps requiring excluded storage classes as partial/unsupported, not browser bugs to bypass.

**Tests:**
- Test logout revoking the shared remote token, clean local child cookie mutation and expired tokens.
- Optional real-account smoke requires separate authorised accounts and never enters CI fixtures.

**Done when:** Cookie copying is not advertised as cloning an account or insulating remote actions.

**Evidence:** `docs/testing/versioned-browsing/VB-078.md`.

#### VB-079 — Publish identity extension capability decision
**Dependencies:** VB-073, VB-074, VB-075, VB-077  
**Design sections:** 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `docs/testing/versioned-browsing/capabilities.json`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/identity-release.md`

**Implementation:**
1. Record exact backend/scope/version evidence and which production identity tasks remain blocked.
2. Enable snapshot branches only if all required capability gates and conditional journey tests actually passed.
3. Otherwise ship diagnostic/negative handling with clean branches still usable.

**Tests:**
- Regression test unsupported general Qt backend cannot enable strict identity by configuration.
- Check all capability claims point to evidence, including conditional tests when enabled.

**Done when:** Release status is explicit: supported scoped identity or evidence-backed unavailable, never implied full support.

**Evidence:** `docs/testing/versioned-browsing/VB-079.md`.

### P7 — Merge planning and conditional application

#### VB-080 — Create immutable cookie merge plan objects
**Dependencies:** VB-015, VB-016, VB-070, VB-077  
**Design sections:** 18.1, 18.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/cookies.rs`
- `crates/ferric-browser-core/src/versioning/merge.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Implementation:**
1. Build plans from complete supported base/parent/child inputs and explicit evaluation time.
2. Store values in memory-only secret handles and expose a redacted immutable plan.
3. Bind plan identity to profile instances, source evidence, scope and expiration.

**Tests:**
- Test attribute-only conflicts, concurrent parent/child divergence and unknown snapshots.
- Verify no preview writes cookies or persists base values.

**Done when:** merge creates a real deterministic plan, never a jar union or hidden mutation.

**Evidence:** `docs/testing/versioned-browsing/VB-080.md`.

#### VB-081 — Add scoped preview and conflict resolution UI
**Dependencies:** VB-080, VB-025  
**Design sections:** 8, 18.2  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`
- `crates/ferric-browser-core/src/action.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-application/src/versioning/projection.rs`

**Implementation:**
1. List grouped affected hosts, additions/deletions/conflicts and opaque metadata.
2. Implement whole-record main/branch/skip choices and precise host/domain filtering.
3. Warn about credential/session coupling without pretending Secure/HttpOnly reliably classifies authentication.

**Tests:**
- Keyboard-only resolution, ambiguous site filtering, untrusted names and expired plan.
- Verify conflict selection does not write to either parent or child.

**Done when:** Users can review proposed supported changes without raw token exposure.

**Evidence:** `docs/testing/versioned-browsing/VB-081.md`.

#### VB-082 — Implement stale-plan and permission revalidation
**Dependencies:** VB-080, VB-081, VB-016  
**Design sections:** 18.3  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Implementation:**
1. Revalidate plan scope, engine/profile instance, current capability and security revisions on apply request.
2. Consume only the matching native confirmation permit; changed inputs invalidate the plan.
3. Keep revision recheck labelled as a precondition, not a substitute for engine write isolation.

**Tests:**
- Test preview/apply parent mutation, child mutation, expired consent and profile replacement.
- Assert no native write effect is emitted before all preconditions hold.

**Done when:** Stale plans cannot apply silently, while remaining engine race limits are explicit.

**Evidence:** `docs/testing/versioned-browsing/VB-082.md`.

#### VB-083 — Qualify or reject controlled cookie write-back
**Dependencies:** VB-074, VB-075, VB-082  
**Design sections:** 18.3, 18.4  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/cookie-apply-boundary.md`

**Implementation:**
1. Investigate the actual backend ability to coordinate external writes and obtain reliable post-write state.
2. Stress workers, in-flight responses, expiry and parent activity during a batch.
3. If adequate apply guarantees cannot be demonstrated, keep main-profile merge unavailable; do not call a Ferric journal an engine transaction.

**Tests:**
- Negative tests show freezing views/interceptor guards/revision-only checks cannot certify atomic application.
- Any positive backend must prove its stated limited or stronger contract under concurrent activity.

**Done when:** The main-profile mutation boundary is either qualified precisely or explicitly blocked.

**Evidence:** `docs/testing/versioned-browsing/VB-083.md`.

#### VB-084 — Implement deterministic mutation executor for a qualified backend
**Dependencies:** VB-082, VB-083  
**Design sections:** 18.4  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.cookie_merge_apply`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/merge.rs`
- `crates/ferric-browser-engine-qt/src/versioning_cookie_adapter.cpp`

**Implementation:**
1. Acquire the qualified write session and re-read supported parent/child scope.
2. Execute ordered per-key operations with expected-before/after values and explicit acknowledgement/read-back.
3. Stop immediately on unknown result, stale data or profile loss; do not claim batch atomicity without backend support.

**Tests:**
- Test insertion, update, deletion, attribute rejection and unexpected external mutation at every step.
- Verify values never leave the secret path in progress/error messages.

**Done when:** merge-apply has verified scoped effects or an explicit partial/unknown result, never blind best effort.

**Evidence:** `docs/testing/versioned-browsing/VB-084.md`.

#### VB-085 — Add conditional compensation and partial-failure recovery
**Dependencies:** VB-084  
**Design sections:** 18.4, 22  
**Type:** implementation; **initial status:** not-started

**Production capability gates:** `versioning.cookie_merge_apply`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Implementation:**
1. Retain only touched-key preimages in memory during application.
2. Compensate only when current state is provably the exact operation-written record; otherwise stop and preserve unrelated writes.
3. Expose Partial/OutcomeUnknown and explicit reload guidance without auto-reloading unsaved pages.

**Tests:**
- Inject failure after every mutation; race a later legitimate parent update before compensation.
- Assert rollback never replaces the entire jar or overwrites unknown intervening changes.

**Done when:** Recovery minimises damage without pretending arbitrary engine writes are reversible.

**Evidence:** `docs/testing/versioned-browsing/VB-085.md`.

#### VB-086 — Track per-key receipts for repeated and partial merges
**Dependencies:** VB-080, VB-015  
**Design sections:** 18.5  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-core/src/versioning/merge.rs`
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Implementation:**
1. Store memory-only per-key reconciliation anchors for verified-applied/converged items.
2. Leave skipped/unresolved keys at their previous base.
3. Do not mutate the child when a user chooses parent; keep that distinction explicit.

**Tests:**
- Test second merge after clean apply, subset apply, skip, chosen parent and later parent changes.
- Verify unresolved items cannot disappear because an entire base snapshot was advanced.

**Done when:** Repeated merges have coherent ancestry without hidden jar synchronisation.

**Evidence:** `docs/testing/versioned-browsing/VB-086.md`.

#### VB-087 — Integrate merge lifecycle with branch teardown and secret cleanup
**Dependencies:** VB-081, VB-082, VB-086, VB-064  
**Design sections:** 17.4, 18  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-application/src/versioning/branch.rs`
- `crates/ferric-browser-application/src/versioning/merge.rs`

**Implementation:**
1. Cancel/expire plans when source/base/profile owner disappears or becomes unsupported.
2. Clear secret bases/deltas/permits at final owner and avoid durable mutation journalling.
3. Refuse branch deletion during unknown apply outcome unless the user explicitly handles the remaining risk.

**Tests:**
- Test closing parent/child during preview/apply, cancellation, profile replacement and late callbacks.
- Audit redaction and bounded retention through repeated plan cycles.

**Done when:** Merge plans cannot outlive their authority or leak authentication history.

**Evidence:** `docs/testing/versioned-browsing/VB-087.md`.

#### VB-088 — Qualify end-to-end merge under supported scope
**Dependencies:** VB-084, VB-085, VB-086, VB-087, VB-078  
**Design sections:** 23, 24.1  
**Type:** qualification; **initial status:** not-started

**Production capability gates:** `versioning.cookie_merge_apply`. An unavailable gate blocks this slice; no fake implementation.

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`

**Planned module paths (create unless introduced by an earlier slice):**
- `crates/ferric-browser-engine-qt/tests/qml/tst_versioning_cookie_merge.qml`

**Implementation:**
1. Run real backend three-way/conflict/scoped deletion/expiry/partial-failure fixtures.
2. Show synthetic coupled-cookie and revoked-token limits in expected results.
3. Record exact scope and engine version; no generic all-cookie/atomic promise.

**Tests:**
- All truth-table plans agree with observed supported backend state.
- No raw tokens enter logs, history, IPC or files; parent unsaved pages are not auto-reloaded.

**Done when:** Reconciliation is enabled only with real supported-backend evidence.

**Evidence:** `docs/testing/versioned-browsing/VB-088.md`.

#### VB-089 — Publish reconciliation status without blocking core release
**Dependencies:** VB-080, VB-081, VB-082, VB-083, VB-086, VB-087  
**Design sections:** 24.1  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `docs/testing/versioned-browsing/capabilities.json`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/merge-release.md`

**Implementation:**
1. Separate implemented pure planner/preview from conditional native executor status.
2. Require VB-084/VB-085/VB-088 evidence before enabling production apply.
3. When blocked, report exact API/guarantee gap and retain usable core/clean branch feature.

**Tests:**
- Test unsupported backend returns MergeApplyUnavailable without any native mutation.
- Review release documentation for accidental claims of successful or atomic whole-jar merging.

**Done when:** The full vision is represented honestly without calling a blocked extension implemented.

**Evidence:** `docs/testing/versioned-browsing/VB-089.md`.

### P8 — Integration, qualification and maintenance

#### VB-090 — Complete configuration/help/IPC and accessibility integration
**Dependencies:** VB-029, VB-041, VB-057, VB-069, VB-079, VB-089  
**Design sections:** 8, 21  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/ferric-browser-config/src/settings.rs`
- `crates/ferric-browser-config/src/validation.rs`
- `crates/ferric-browser-core/src/completion.rs`
- `crates/ferric-browser-runtime/src/ipc_projection.rs`
- `crates/ferric-browser-engine-qt/qml/components/FerricVersioningPanel.qml`

**Implementation:**
1. Generate discoverable commands/help/capabilities from the central registries.
2. Finish config validation, bounded IPC query output and keyboard/assistive labels.
3. Check default-off rollout and unavailable advanced operations across all entry points.

**Tests:**
- Run keyboard-only preserve/stash/branch/restore journeys and contrast/focus checks using existing UI conventions.
- Test identical permission/target behaviour through commands, switcher and IPC.

**Done when:** The feature is a coherent Ferric workflow rather than a disconnected command collection.

**Evidence:** `docs/testing/versioned-browsing/VB-090.md`.

#### VB-091 — Run storage/lifecycle/privacy fault-injection campaign
**Dependencies:** VB-090, VB-026, VB-039, VB-056, VB-064, VB-087  
**Design sections:** 22, 23  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/testing/versioned-browsing/README.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/fault-campaign.md`

**Implementation:**
1. Exercise crash/restart/disk-full/profile-close/renderer-loss at each operation boundary.
2. Audit private/ephemeral exports, archive isolation and secret redaction across logs and protocol fixtures.
3. Record unsupported native checks as release blockers for that capability.

**Tests:**
- No visible half-valid commit, cross-profile callback adoption or false LIVE recovery.
- No blind cookie compensation or automatic credentials persistence.

**Done when:** Failure behaviour meets the invariants across the complete supported feature.

**Evidence:** `docs/testing/versioned-browsing/VB-091.md`.

#### VB-092 — Measure latency, retained resources and cleanup
**Dependencies:** VB-090, VB-041, VB-057, VB-069  
**Design sections:** 21  
**Type:** qualification; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `crates/xtask/src/main.rs`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/performance.md`

**Implementation:**
1. Measure command responsiveness, archive capture impact, memory/profile counts and repeated cleanup on the target low-resource Linux machine.
2. Keep engine/RSS attribution honest and distinguish shared processes from per-tab usage.
3. Adjust proposed bounds only with recorded evidence; do not remove them to hide failures.

**Tests:**
- Run long repeated stash/branch/capture/drop cycles and metadata queries during blob I/O.
- Verify no unbounded event/secret/retention growth and no UI-thread file hashing.

**Done when:** Resource cost is visible and acceptable for the declared target, with documented limits.

**Evidence:** `docs/testing/versioned-browsing/VB-092.md`.

#### VB-093 — Add upgrade/requalification and compatibility regression policy
**Dependencies:** VB-090, VB-002  
**Design sections:** 6, 7, 23.4  
**Type:** implementation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/architecture/ADR-0003-system-qt-distribution.md`
- `.github/workflows/ci.yml`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/engine-compatibility.md`

**Implementation:**
1. Define capability certificate invalidation on relevant Qt/Chromium/backend changes.
2. Run preserved format fixtures across the declared supported and transition runtime where available.
3. Document safe disabling of native capabilities without destroying stored commits.

**Tests:**
- Test unknown engine fingerprint, older/newer manifests and unsupported archive backend.
- Verify unavailable runtime support does not cause silent conversion to weaker fidelity.

**Done when:** Maintenance has an explicit requalification path rather than a fragile version-number assumption.

**Evidence:** `docs/testing/versioned-browsing/VB-093.md`.

#### VB-094 — Perform final scope/implementation/evidence reconciliation
**Dependencies:** VB-091, VB-092, VB-093  
**Design sections:** 24, 26  
**Type:** review; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `docs/requirements.csv`
- `CONTRIBUTING.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/release-review.md`

**Implementation:**
1. Audit all slice statuses, invariants, capabilities and remaining conditional blockers.
2. Run repository quality gates and review new dependencies/licences/security-sensitive changes.
3. Separate core-complete, identity-complete and reconciliation-complete verdicts.

**Tests:**
- Every enabled capability points to native evidence where required.
- No task is marked verified merely because a stub returns Unsupported or a mock passed.

**Done when:** The release claim exactly matches working supported behaviour and explicitly names unavailable vision elements.

**Evidence:** `docs/testing/versioned-browsing/VB-094.md`.

#### VB-095 — Publish maintainer and next-agent operational handoff
**Dependencies:** VB-094  
**Design sections:** 25, 26  
**Type:** documentation; **initial status:** not-started

**Paths to inspect or extend (baseline or earlier slice):**
- `README.md`
- `CONTRIBUTING.md`
- `docs/DEVELOPMENT_SPEC.md`

**Planned module paths (create unless introduced by an earlier slice):**
- `docs/testing/versioned-browsing/maintenance.md`

**Implementation:**
1. Document supported commands, safety/fidelity boundaries, retention/deletion and upgrade testing.
2. List conditional tasks still blocked with precise prerequisites, not a promise that future work is done.
3. Provide concise next-agent entry points and current implementation/evidence SHAs. Do not publish externally without authority.

**Tests:**
- Verify documentation examples against command registry output.
- Validate backlog references/dependencies and all linked local evidence paths.

**Done when:** A smaller agent or new maintainer can continue safely without the original chat context.

**Evidence:** `docs/testing/versioned-browsing/VB-095.md`.

### Explicitly separated safety and qualification slices (2026-10-07)

#### VB-096 — Qualify current-window restore replacement and consent
**Dependencies:** VB-024, VB-033  
**Design sections:** 11.3, 12.1  
**Type:** implementation; **initial status:** blocked

**Implementation:** Preview displaced work, require explicit consent and qualified live retention; allocate fallback and revalidate the exact owner before replacement. A URL checkpoint cannot preserve unknown unsaved state.

**Tests:** Native dirty-editor, active WebRTC/media, cancellation, stale target, allocation failure and owner-loss journeys must show safe refusal or genuine recoverability.

**Done when:** Current-window replacement is consented and safely reversible, or refuses without mutation.

**Evidence:** `docs/testing/versioned-browsing/VB-096.md`.

#### VB-097 — Qualify ordinary viewless profile download to durable ledger
**Dependencies:** VB-024, VB-044  
**Design sections:** 13.2, 23.4  
**Type:** qualification; **initial status:** blocked

**Implementation:** Obtain a public content-originated null-page request on the exact Quick profile, route request and terminal callback through its profile owner, and prove its durable ledger row and installed bytes.

**Tests:** Standalone exact-profile null-page probe, then integrated Ferric completion/cancellation and teardown. Do not substitute a page-backed request or run known-crashing extension activation by default.

**Done when:** A genuine ordinary viewless download is durably accounted for without unsafe view access.

**Evidence:** `docs/testing/versioned-browsing/VB-097.md`.

#### VB-098 — Qualify hidden-parent retention for clean branches
**Dependencies:** VB-059, VB-033, VB-036  
**Design sections:** 12.1, 16.2  
**Type:** implementation; **initial status:** blocked

**Implementation:** Admit only qualified parent views into exact profile/view leases after child and fallback allocation; hide or freeze only when activity absence and lifecycle ownership are proven.

**Tests:** Native dirty-editor, WebRTC/media, popup, download, renderer-loss and owner-destruction matrices; refusal leaves the visible parent usable.

**Done when:** A clean branch can hide its parent and later recover that exact live view without mislabelling an active or lost view.

**Evidence:** `docs/testing/versioned-browsing/VB-098.md`.

#### VB-099 — Extract and pin a production-capable native engine backend
**Dependencies:** VB-001, VB-002  
**Design sections:** 4, 20, 23.4  
**Type:** implementation; **initial status:** not-started

**Implementation:** Extract a minimal Qt/Chromium patch from the isolated research overlays, remove `ForProbe` and diagnostic controls, and bind exact Quick view/profile handles to Ferric owner/generation tokens. Build and explicitly select a fingerprinted, sandbox-preserving Linux development backend while public Qt stays the default. The 2026-10-08 disposable verifier is preparatory, not this production patch.

**Tests:** Clean-source zero-fuzz build; source/binary provenance, ABI/runtime-loader and mixed-bundle refusal; owner loss, renderer failure, sandbox checks and absence of test-only APIs.

**Done when:** A non-diagnostic pinned backend is explicitly selectable for native qualification without enabling a strict capability or replacing the public default.

**Evidence:** `docs/testing/versioned-browsing/VB-099.md`.

#### VB-100 — Qualify exact-view WebRTC and media absence lease on the pinned backend
**Dependencies:** VB-099  
**Design sections:** 11, 20  
**Type:** qualification; **initial status:** not-started

**Implementation:** Connect the engine's retained, invalidatable lease to the exact Ferric view/profile generation at stash admission and final pre-hide recheck. Cover peers, media/capture, every frame, script dispatch and lifecycle loss; gaps yield `Unknown`.

**Tests:** Native data-only, camera, microphone, screen, cross-origin child, queued-script and owner/process-loss races; active/unknown activity must refuse without hiding the source.

**Done when:** The exact production view has a bounded full-activity `Clear` certificate for the declared scope; otherwise live retention stays unavailable.

**Evidence:** `docs/testing/versioned-browsing/VB-100.md`.

#### VB-101 — Qualify canonical cookie snapshot and fresh-child seed on the pinned backend
**Dependencies:** VB-099  
**Design sections:** 18, 20  
**Type:** qualification; **initial status:** not-started

**Implementation:** Connect an engine-issued complete canonical source snapshot and T0 to an exact owned profile. Seed only an unpublished empty child under exclusive ownership, check every result and exact read-back, discard failures and protect secret buffers. Live merge needs a separate all-writer conditional-apply proof.

**Tests:** Response/script/service-worker writer races, partition/legacy/expiry identity, network-service loss, partial writes, restart and child-publication faults; no secret values in QML/logs/artifacts.

**Done when:** Snapshot and seed/read-back have independent pinned-backend certificates for the declared scope; merge remains disabled without its own proof.

**Evidence:** `docs/testing/versioned-browsing/VB-101.md`.

#### VB-102 — Provide and qualify a pinned-backend ordinary viewless download origin
**Dependencies:** VB-099, VB-044  
**Design sections:** 13.2, 23.4  
**Type:** qualification; **initial status:** not-started

**Implementation:** Produce a bounded content-originated null-view request from the exact Quick profile without MV3 activation or a page-owned substitute; route its callbacks through the profile owner to installed bytes and the durable ledger.

**Tests:** Standalone null-view exact-profile witness followed by integrated completion/cancel/owner-loss, duplicate callback, cross-profile and restart journeys.

**Done when:** A genuine ordinary viewless request is accounted for by the exact durable profile ledger; otherwise VB-097 stays blocked.

**Evidence:** `docs/testing/versioned-browsing/VB-102.md`.
