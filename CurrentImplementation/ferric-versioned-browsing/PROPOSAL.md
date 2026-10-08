# Ferric Versioned Browsing
## Detailed design and agent implementation proposal

**Document version:** 0.1.0  
**Status:** proposed; implementation and native qualification not performed  
**Prepared:** 27 September 2026  
**Repository:** `thomasaiwilcox/ferric-browser`  
**Inspected main:** `deea458d92bcf6d8fbea7b04af17823b5882cc1b`  
**Inspected tree:** `f56fd674cd0eefbae389355db4068afc915b6ac1`  
**Baseline commit:** `docs: replace implementation proposals with ADR`  
**Audience:** maintainer, implementation agents, reviewers and Qt runtime qualification contributors

> Preserve useful browser work. Retain live pages when that is the right trade-off. Create isolated lines of experimentation. Explain differences and restoration limits. Reconcile only state whose meaning and application Ferric can validate.

This proposal retains the agreed product vision while separating implementable public-API features from conditional capabilities. It is not evidence that Ferric already implements Versioned Browsing, that native tests have passed, or that a complete Chromium cookie snapshot is available through today's Qt API.

The companion `IMPLEMENTATION_BACKLOG.json` is the execution ledger. Appendix B renders the same slices for human readers. `AGENT_HANDOFF.md` specifies how a goal-mode agent should advance one slice at a time. The baseline and source inventory are in `BASELINE_AUDIT.json` and Appendix A.

## Contents

- [1. Product contract](#1-product-contract)
- [2. Correctness boundaries discovered during research](#2-correctness-boundaries-discovered-during-research)
- [3. Alignment with the inspected main branch](#3-alignment-with-the-inspected-main-branch)
- [4. Normative invariants and ADR changes](#4-normative-invariants-and-adr-changes)
- [5. Identity, state and lifetime model](#5-identity-state-and-lifetime-model)
- [6. Fidelity and capability reporting](#6-fidelity-and-capability-reporting)
- [7. Public Qt/Chromium API plan](#7-public-qtchromium-api-plan)
- [8. Command and action contract](#8-command-and-action-contract)
- [9. Operation protocol and reducer integration](#9-operation-protocol-and-reducer-integration)
- [10. Durable storage design](#10-durable-storage-design)
- [11. Structural commit, diff and restore](#11-structural-commit-diff-and-restore)
- [12. Live stash and retention implementation](#12-live-stash-and-retention-implementation)
- [13. Durable document capture](#13-durable-document-capture)
- [14. Bounded semantic page state](#14-bounded-semantic-page-state)
- [15. Historical archive viewing](#15-historical-archive-viewing)
- [16. Isolated branch creation and switching](#16-isolated-branch-creation-and-switching)
- [17. Cookie backend and observation model](#17-cookie-backend-and-observation-model)
- [18. Three-way cookie merge](#18-three-way-cookie-merge)
- [19. Other state classes and explicit exclusions](#19-other-state-classes-and-explicit-exclusions)
- [20. Security, privacy and data handling](#20-security-privacy-and-data-handling)
- [21. Resource limits and operational defaults](#21-resource-limits-and-operational-defaults)
- [22. Error and recovery semantics](#22-error-and-recovery-semantics)
- [23. Validation and qualification strategy](#23-validation-and-qualification-strategy)
- [24. Implementation phases and release milestones](#24-implementation-phases-and-release-milestones)
- [25. Agent execution protocol](#25-agent-execution-protocol)
- [26. Maintainer acceptance checklist](#26-maintainer-acceptance-checklist)
- [Appendix A. Sources and baseline audit](#appendix-a-sources-and-baseline-audit)
- [Appendix B. Dependency-ordered implementation slices](#appendix-b-dependency-ordered-implementation-slices)

---

## 1. Product contract

### 1.1 The feature

Ferric Versioned Browsing makes browser working state deliberate rather than disposable. The primary vocabulary is **commit, stash, branch, switch, diff, merge and restore**. A user can use commit and restore without learning the rest.

A commit is an immutable durable record. A stash is a temporary owner of retained live pages. A branch is a mutable line of work with ancestry and its own engine profile. Switching selects a line of work; restoring materialises saved state. Diff compares typed state. Merge reconciles only specifically supported state classes, initially cookies when the necessary backend capabilities are established.

The central user journeys are:

1. Preserve a research window, rearrange and close tabs freely, then restore its structure without losing the current window.
2. Put an eligible live editor or web application aside without unloading it; later resume the same page instance.
3. Preserve a parent and open an isolated experimental child. Discard the child or continue using it without contaminating the parent's local cookie/storage partition.
4. Inspect saved historical documents rather than silently replacing them with today's versions at the same URL.
5. Where supported, initialise a branch from the parent's cookie state, inspect subsequent differences, and deliberately apply selected changes back after conflict review.

**A branch is not a sandbox for remote actions.** Purchases, messages, cloud administration, server-side logout, autosave and API operations remain real. This warning belongs in branch creation and merge documentation, not merely in developer notes.

### 1.2 What stays out

No separate `clone` operation; ordinary tab duplication remains ordinary tab duplication. No shared-profile branch in this feature. No generic DOM diff, arbitrary application-state merge, IndexedDB merge, cookie-header emulator, browser-engine fork, private Qt API, Chromium database manipulation, renderer/OS checkpointing or V8 heap cloning. No branch-graph visualisation requirement. No Git dependency, remote synchronisation or cloud service.

Do not expand this project into implementing the separate tab-tag/resource-pool vision. Existing metadata can be included; future typed metadata can be added through a new schema. Current `TabState` has no general tab-tag or CPU-pool field. [R4]

### 1.3 Deliverable classes

| Class | Included work | Release rule |
|---|---|---|
| Foundation | Structural commits, safe restore, semantic diff, bounded journal, retention | Ship after normal test and storage fault gates |
| Live continuity | Stash, retained view leases, resume, loss reporting | Ship only on qualified Qt/Wayland runtime |
| Durable page continuity | Document archives, opt-in editable sidecars, conservative reconstruction | Ship after capture privacy and archive isolation gates |
| Isolated exploration | Clean branches, ancestry, switch, disposal, explicit capture/export | Ship independently of cookie snapshot support |
| Conditional identity | Snapshot-seeded cookie branches | Enable only for a backend and scope whose identity, completeness and read-back guarantees pass |
| Conditional reconciliation | Cookie three-way planner and controlled merge application | Planner may ship in tests; main-profile writes stay disabled until apply prerequisites pass |

These are milestones for **this feature**, not a redefinition of the repository's global V1/V1.1/M0–M4 requirements. A completed negative feasibility investigation does not mean a capability is supported.

---

## 2. Correctness boundaries discovered during research

### 2.1 Live retention is not an immutable memory snapshot

Qt's Frozen lifecycle retains a browsing context while suspending most task processing; freeze/resume callbacks and the surrounding environment still matter. [Q2] Therefore `LIVE` means **the same page instance is still available**, not “all memory and external state are fixed at an exact timestamp.”

Resuming a stash continues that instance. Once the page evolves, the old live state is not separately preserved. Repeated returns to a fixed historical base require a durable representation and reconstruction. Retaining a pointer to a page is not equivalent to committing it.

The parent profile can change because other parent tabs remain active. Servers can revoke tokens or change data. Browser time continues, and resumed applications may immediately refresh. The UI must not call the parent universally “untouched” or promise zero network activity merely because its view was frozen.

### 2.2 A reconstructed child is not a cloned execution context

A new branch receives a new `WebEngineView`, new runtime IDs and a separate engine profile. It may load the same URL and receive supported form/scroll state. It does not inherit closures, framework stores, JavaScript object identity, promises, sockets, workers, WebAssembly memory or GPU state. Public Qt page saving and script evaluation do not supply a renderer clone operation. [Q1, Q3, Q4]

### 2.3 Cookie snapshot and merge are not unconditional Qt features

The public cookie store provides asynchronous mutation and observation, but no documented enumeration-complete event or batch transaction. `QNetworkCookie` does not expose a Chromium partition key. [Q6, Q7] A Rust mirror cannot restore metadata the adapter never received.

A quiet timer, a sentinel cookie, repeating enumeration until two lists match, or freezing visible pages is **not proof** of a complete, atomic source snapshot. A locally assigned revision is an observation revision, not a Chromium transaction ID. Do not call it exact `T0` unless a qualified backend supplies the necessary boundary.

Missing partition identity also means the implementation cannot assume it can detect and skip every partitioned cookie. If two engine cookies collapse into the same exposed key, ambiguity exists before the merge algorithm runs. Full-profile snapshot seeding and write-back remain unavailable on a lossy backend. Investigation and an explicitly labelled diagnostic mirror remain useful.

### 2.4 Captured documents are sensitive

Omitting password fields from a sidecar does not remove secrets from the HTML, embedded JSON, hidden controls, subresources or captured document. Captures must be treated as potentially credential-bearing local files. Sensitive capture is explicit; crash recovery stays descriptor-only. [R5]

### 2.5 A staged operation is not an engine transaction

Ferric can atomically publish its own manifests and guard its own commands. It cannot claim atomicity for asynchronous writes to Chromium's cookie store or a group of web pages. Revalidation reduces races; it does not create compare-and-swap in the engine. Failure states must include partial application and unknown outcomes.

---

## 3. Alignment with the inspected main branch

### 3.1 Existing architecture: retain, do not bypass

| Existing location | Verified responsibility | Integration rule |
|---|---|---|
| `crates/ferric-browser-core/src/model.rs` | Private application state; read-only public projections; tabs/windows/profile identity | Add domain fields behind reducer ownership |
| `crates/ferric-browser-core/src/reducer.rs` | Physical reducer file, logically a child of `model` | Add typed events/effects here or child modules; do not invent `model/reducer.rs` |
| `crates/ferric-browser-core/src/command.rs` | Lexer, registry, command argument metadata | Extend existing registry and one typed argument conversion |
| `crates/ferric-browser-core/src/action.rs` | Namespaced actions, confirmation, sources, capability metadata | Add versioning subjects and predicates, not a second action system |
| `crates/ferric-browser-runtime/src/lib.rs` | `BrowserRuntime`, `RuntimeInput`, `RuntimeEffect`, immutable `RuntimeView` | Transport new typed domain transitions and observations |
| `crates/ferric-browser-application/src/lib.rs` | `BrowserApplication`: runtime + configuration + durable-work coordination | Own versioning operation orchestration at this boundary |
| `crates/ferric-browser-application/src/storage.rs` | `StorageCoordinator`, `StorageTicket`, bounded scheduling | Extend typed request/completion handling; preserve correlation |
| `crates/ferric-browser-storage/src/sessions.rs` | Schema-2 safe recovery descriptors and atomic generations | Keep recovery format separate from explicit versioned captures |
| `crates/ferric-browser-storage/src/worker.rs` | Single profile metadata owner, typed `StorageCommand`/completion | Extend all enum/dispatch/completion paths together |
| `crates/ferric-browser-engine-qt/src/browser_ui_sessions.rs` | Current adapter integration for session/context restore | Reuse safe-descriptor behaviour; do not add another persistence owner |
| `crates/ferric-browser-engine-qt/qml/Main.qml` | Composition-only entry point | No feature state machine here |
| `qml/components/FerricBrowserRuntimeSurface.qml` under engine crate | View-pool reconciliation and engine action routing | Retained views must not be destroyed as unmatched visible tabs |
| `qml/components/FerricWindowRegistryController.qml` under engine crate | Ephemeral profile token references and window-owner counts | Add explicit retention ownership; no window-only lifetime assumption |
| `qml/components/FerricBrowserRuntimePresentation.qml` under engine crate | Existing ephemeral window opening | Reuse profile-compatible host creation, with typed coordinator control |
| `src/request_interceptor.h/.cpp` under engine crate | Public `QQuickWebEngineProfile` attachment and request policy | Compose policy; never replace a live interceptor accidentally |
| `crates/ferric-browser-engine-qt/build.rs` | Collects QML/JS resources recursively; explicit C++ source list | New C++ adapters require registration; scripts join existing resource manifest |

Source details are pinned in Appendix A. Some older testing documents still refer to `Main.qml` as the registry owner; the current entry point is only `FerricPrimaryBrowserWindow {}`. Implement against current code, not those stale location descriptions. [R1–R14]

The workspace already separates core, runtime, application, config, IPC, storage and the Qt adapter. Do not add a new top-level crate by default. CXX-Qt is pinned to 0.10.0, the project uses UUID v4 for durable IDs, and Qt is discovered as a system dependency. Do not assume UUID v7, a hash crate or a crypto crate is already a workspace dependency. [R1]

### 3.2 Specific integration constraints

**Window/profile affinity:** current windows have one `ProfileId`; cross-profile live tab movement is rejected. Isolated branches initially use another same-purpose native browser window with their own `BrowserApplication` owner. `switch` focuses the appropriate host through existing activation mechanisms. A mixed-profile tab strip would be a separate architectural change, not an incidental implementation shortcut. [R4, R6]

**Freeze eligibility:** ordinary `tab-suspend` currently rejects dirty or unknown editable state. A user-requested preservation stash needs its own typed intent and consent path. Do not weaken the safety rules of automatic lifecycle management or discard to make a stash work. [R9]

**Storage scheduling:** current application scheduling correlates active requests by `StorageOperation`. Do not submit several same-kind capture requests concurrently unless correlation is intentionally redesigned. Use one in-flight metadata operation of each kind and ticket every result. [R8]

**Runtime IDs:** restored sessions intentionally allocate new runtime tab IDs. Durable logical page identity belongs in versioning manifests and a mapping; it is not a license to reuse live IDs. [R10]

**Scroll ownership:** ADR-0014 makes the explicit scroll target document-local, using weak references in ApplicationWorld. Durable restoration starts with `auto`. Capturing numeric positions does not persist the selected scroll target or reinterpret an old weak reference/selector as authority. [R11]

### 3.3 Proposed new files and module ownership

All paths in this subsection are **new proposals**, not existing repository facts.

- `core/src/versioning/{mod,identity,model,capture,diff,merge,commands,limits,error}.rs`: pure domain types, validation, plans and algorithms. State-mutating integration remains under the reducer.
- `application/src/versioning/{mod,coordinator,capture,restore,stash,branch,cookies,merge,projection}.rs`: orchestration, permits, receipts, timeouts and coordination with storage.
- `storage/src/versioning/{mod,schema,manifest,objects,journal,retention}.rs`: explicit-capture DTOs, validated storage and queries.
- `engine-qt/src/browser_ui_versioning.rs`: thin Rust/Qt translation; never a parallel source of browser state.
- `engine-qt/src/versioning_cookie_adapter.h/.cpp`: narrowly expose public Quick profile cookie observation/mutation.
- `engine-qt/qml/components/FerricVersioningViewHost.qml`: retain/adopt/release views under typed leases.
- `engine-qt/qml/components/FerricVersioningPanel.qml`: one keyboard-accessible list/detail/preview surface.
- `engine-qt/qml/scripts/VersionedPageState.js`: bounded capture and restoration helpers in ApplicationWorld.
- `docs/testing/versioned-browsing/`: slice evidence and qualification records.

Use the `crates/ferric-browser-` prefix appropriate to each module abbreviation above. Split further only when the implementation requires it; these names describe responsibilities, not permission to create empty scaffolding across the whole tree.

---

## 4. Normative invariants and ADR changes

`MUST` is a required property of the proposed implementation. Numerical limits are project defaults proposed here, not measured Qt limits.

| ID | Required invariant |
|---|---|
| VB-I01 | Only reducer events mutate browser/versioning domain state. Application code coordinates; adapters report facts. |
| VB-I02 | Every delayed operation identifies its application owner, engine profile instance, tab generation and document generation. Current focus is never substitute authority. |
| VB-I03 | A retained live view never crosses engine profiles; a branch child is a fresh view. |
| VB-I04 | Ordinary crash recovery remains safe-descriptor-only. No page or cookie capture is triggered by recovery checkpoint scheduling. |
| VB-I05 | Private/ephemeral data never enters a normal durable store automatically. Explicit export is a distinct, confirmed operation. |
| VB-I06 | No user-facing full snapshot claim can be based on an incomplete or identity-lossy cookie mirror. |
| VB-I07 | No private Qt/Chromium API, database scraping or debugging endpoint is introduced as an unreviewed fallback. |
| VB-I08 | No remote action rollback, execution-memory clone or generic application-state merge is promised. |
| VB-I09 | A completed commit references only validated, durably published objects. Failure cannot publish a half-valid commit. |
| VB-I10 | Retention quality, reconstruction coverage and consistency are independent facts; uncertainty is not labelled success. |
| VB-I11 | Live stash eviction is explicit. Memory pressure must not silently discard the only copy of unsaved live work. |
| VB-I12 | Cookie values and editable contents are excluded from logs, ordinary IPC, command history and routine diff output. |
| VB-I13 | A requested snapshot branch never silently becomes shared-profile or clean-profile. |
| VB-I14 | Restore does not replay POST, submit forms, click controls, grant permissions or run captured JavaScript. |
| VB-I15 | The page is untrusted data. A sidecar, archive or script result cannot choose paths, native commands, profiles or permission grants. |
| VB-I16 | View destruction precedes final engine-profile destruction; hidden owners and pending callbacks count. |
| VB-I17 | No native capability is marked verified by source-string checks, mocks or compilation alone. |
| VB-I18 | Timeouts, cancellation and unknown mutation outcomes terminate in explicit recoverable states, not indefinite spinners. |

Propose new ADRs using the next available numbers at implementation time; do not hard-code a number that may be occupied:

**Versioned browsing ownership and explicit capture.** Preserve ADR-0006 safe recovery, define the new capture namespace, acknowledge secret-bearing archives, and define explicit export from a transient branch. Ordinary private profile export remains disabled in this proposal.

**Branch profiles and retention leases.** Extend ADR-0011/0012 for explicit parent–child data seeding and retained owners without permitting arbitrary cross-profile tab moves or context membership.

**Cookie backend capability contract.** Record exact snapshot and write-back guarantees, tested engine version, unavailable metadata, and the default-off policy for unsupported strict semantics.

ADR-0014 remains unchanged: the durable format does not save active explicit-scroll-target identity. Accept these product boundaries before implementing a conflicting slice. [R5, R6, R7, R11]

---

## 5. Identity, state and lifetime model

### 5.1 Durable identity versus runtime identity

Use repository-style opaque runtime IDs for live objects and UUID v4 newtypes for stored versioning records. A durable `LogicalPageId` identifies one conceptual page slot across commits. A child branch has a new runtime `TabId` and an inherited logical lineage reference. A user deliberately duplicating a page gets a new logical ID with optional `derived_from`, not two simultaneously live tabs with the same ownership identity.

Proposed types, illustrative rather than copy/paste-complete Rust:

```rust
struct CommitId(Uuid);
struct BranchId(Uuid);
struct StashId(Uuid);
struct LogicalPageId(Uuid);
struct RetainedViewId(Uuid);
struct OperationId(Uuid);
struct ProfileInstanceId(Uuid);   // random per actual engine-profile lifetime
struct ApplicationOwnerId(Uuid); // prevents collisions between window runtimes

struct CapturedOwner {
    application: ApplicationOwnerId,
    runtime_profile: ProfileId,
    profile_instance: ProfileInstanceId,
    window: WindowId,
    target: Target,              // existing tab + generation + document
}
```

Durable profile UUIDs refer only to normal profile storage ownership. Do not serialize `ProfileInstanceId`, Qt pointers, runtime tab IDs or secret-vault handles as sufficient authority after restart. Stored IDs are references requiring validation, not capabilities.

### 5.2 Domain records

```text
VersioningState
  lines: BranchId -> BranchRecord
  retained: RetainedViewId -> RetentionRecord
  active operations: OperationId -> OperationState
  current line per application owner
  bounded commit/list projections

BranchRecord
  id, display name, parent branch, fork commit/reference
  requested inheritance, actual inheritance report
  owner normal-profile UUID (if any)
  engine profile instance (live only)
  logical page membership
  working revision, last commit
  state: preparing | live | retained | closed | lost

RetentionRecord
  id, branch/stash owner, CapturedOwner
  keep-alive lease count
  observed lifecycle: active | frozen | discarded | lost
  original host and restoration placement
  optional independent durable fallback reference
```

Actual QObject/QQuickItem handles belong in the Qt adapter registry, keyed by `RetainedViewId`. Rust stores metadata and controls transitions. QML references alone are not sufficient ownership: parent destruction must not destroy a view that has a valid retention lease.

### 5.3 A branch owns working state, not an immutable page

The root line `main` means the source working environment inside a particular lineage, not Git's repository branch. Names resolve within owner profile and lineage; UUIDs disambiguate. Nesting is allowed up to a configured depth. No branch can point to itself or introduce ancestry cycles.

Creating a branch captures a fork base and optionally retains the parent live instance. Switching away may freeze eligible pages. Switching back resumes them. **After resuming and editing a parent, it no longer represents the original fork time.** The fork commit still does.

Commits can survive branch deletion if explicitly pinned or referenced. Stashes do not become commits automatically. Dropping a branch with an only-live stash requires a loss confirmation. Keeping a child does not require cookie merge: users may continue using its live profile or explicitly export a structural/page commit.

### 5.4 Scope

Initial commands support `--scope=tab` and `--scope=window`. A window scope contains one profile by existing design. Later `--scope=context` collects only validated normal-profile members of that context. No implicit all-profile workspace capture and no compositor layout serialization.

A multi-window capture is a set of owner-consistent captures with individual intervals; it is not a globally simultaneous browser/website snapshot. Multi-profile context input is rejected rather than coalesced.

---

## 6. Fidelity and capability reporting

Do not implement a single ranked enum that equates an archive with a reconstructed live app. Use independent dimensions:

```text
retention:       live-frozen | live-active | none | lost
representation: structural | archive | semantic-page
coverage:        complete-for-request | partial | unavailable
consistency:     owner-consistent | capture-interval | observed-only | unknown
identity:        clean | snapshot-verified | unsupported
persistence:     durable | memory-only
```

“Complete” is always relative to the **declared bounded capture request**, not all possible web-platform state. A document archive can be durable while form coverage is unavailable. A live stash can remain available while its old authentication is no longer valid.

A UI summary may use `LIVE`, `ARCHIVED`, `RECONSTRUCTABLE`, `STRUCTURAL` and `PARTIAL`, but the detail view and IPC preserve these dimensions. Suggested messages:

```text
stash editor: LIVE / frozen / memory-only
Same page retained. Closing Ferric or losing its renderer loses this live copy.

commit research: STRUCTURAL + ARCHIVE / 7 of 9 pages captured
2 unavailable: loading document; unsupported embedded content.

branch trial: isolated / clean / page reconstruction partial
Original retained; authentication was not copied. Remote actions remain real.
```

Capabilities are structured facts with `supported | unsupported | unqualified`, evidence reference, Qt/Chromium fingerprint and reason code. Set them from explicit adapter probes and qualification, not from a feature flag alone. Key names include:

`versioning.structural`, `versioning.live_retention`, `versioning.archive_capture`, `versioning.archive_offline`, `versioning.semantic_restore`, `versioning.clean_branch`, `versioning.cookie_identity`, `versioning.cookie_snapshot`, `versioning.cookie_seed_verify`, `versioning.cookie_merge_apply`.

Capability records distinguish **primitive gate status** from **production enablement**. A positive isolated API probe can permit implementation and integration testing; it does not enable the product. Production enablement additionally requires the relevant release-milestone certificate. Backlog `required_capabilities` refers to the primitive gate, while user commands require both that gate and the finished product qualification. This prevents a circular requirement to ship a feature before testing its implementation.

The release configuration may expose the broad vision while honestly showing conditional capabilities as unavailable. A negative gate has a documented result and regression test; it is not a reason to remove independent functionality.

---

## 7. Public Qt/Chromium API plan

### 7.1 API inventory

This inventory targets the inspected system-Qt architecture and the Qt 6.11 documentation family. The implementation must record the installed Qt patch, QtWebEngine build and embedded Chromium/security patch versions. An upstream documentation version is not runtime qualification. [Q8]

| Operation | Public API | Surface / availability | Completion and restrictions | Feature |
|---|---|---|---|---|
| Freeze/resume | `WebEngineView.lifecycleState`, `recommendedState` | QML; available on baseline | Observe property transition; keep visible view Active | Stash, switching |
| Renderer loss | `renderProcessTerminated`, `renderProcessPid` | QML | Instance/generation-scoped signals; PID not exclusive tab ownership | Retention loss, diagnostics |
| Page archive | `WebEngineView.save(path, format)` | QML, since Qt 6.6 | Asynchronous page-save download; controlled path before invocation | Page commit |
| Download result | `WebEngineDownloadRequest.state`, `isFinished`, `interruptReason`, `receivedBytes`, `isSavePageDownload` | QML/public C++ equivalent | Finished is not necessarily successful; page saves auto-start | Capture validation |
| Script evaluation | `WebEngineView.runJavaScript(script, worldId, callback)` | QML | Bounded plain-data result; no Promise/function result contract | Semantic capture/restore |
| Script injection | `WebEngineScript` injection point, world, `runsOnSubFrames` | QML/C++ | DOM shared, JS environments separated | Per-document helper |
| Frame traversal | `mainFrame`, `webEngineFrame.children`, `isValid`, `url`, `runJavaScript` | Frame value type Qt 6.8; QML `children` Qt 6.10 | Children unordered; frame can vanish before callback | Bounded frame support |
| Quick profiles | `WebEngineProfilePrototype.instance()` | QML, Qt 6.9 | New prototype/instance per branch; empty storage name for OTR | Clean branch |
| Native profile access | `QQuickWebEngineProfile::cookieStore()` | Public C++ | Correct Quick class; do not recover a private page pointer | Cookie adapter |
| Cookie observation | `loadAllCookies`, `cookieAdded`, `cookieRemoved` | Public C++ | Asynchronous; no documented full-enumeration completion | Diagnostic mirror/gate |
| Cookie mutation | `setCookie`, `deleteCookie` | Public C++ | No batch transaction/CAS; origin/domain mapping matters | Conditional seed/merge |
| Cookie representation | `QNetworkCookie` accessors / `hasSameIdentifier` | Public C++; SameSite since Qt 6.1 | Bytes for name/value; incomplete Chromium metadata | Limited descriptor backend |
| Archive network gate | `QWebEngineUrlRequestInterceptor::interceptRequest` and Quick profile attachment | Existing C++ adapter | Request gate, not a cookie overlay or full network transaction lock | Offline archive host |
| Profile deletion | QObject/profile lifetime + existing owner registry | QML/C++ | Cancel jobs; destroy views before last profile lease | Branch disposal |
| Runtime version | `qWebEngineChromiumVersion`, `qWebEngineChromiumSecurityPatchVersion` | QtWebEngine public global functions | Record installed runtime, not guessed Chrome release | Qualification |

Sources: [Q1–Q9, Q12]. Specific source comments and compile checks must confirm signatures on the chosen patch before exposing a capability.

### 7.2 APIs that are not equivalent

The Widgets `QWebEnginePage` API is useful documentation but a QML `WebEngineView` does not expose its internal page as a public pointer. Use its QML methods and public Quick profile APIs. `toHtml`, PDF output and screenshots are not live application snapshots. Session history serialisation is not a supported general replacement for capture; do not reverse-engineer Chromium PageState/BFCache blobs. [Q1, Q8]

### 7.3 CDP position

Chrome DevTools Protocol exposes richer snapshot, runtime and storage inspection than Qt's basic cookie wrapper, including cookie partition metadata in current protocol schemas. [Q10, Q11] It is a **research alternative**, not the production default of this proposal.

Do not enable a remote-debugging port just to make cookie tests pass. Qt's documented remote-debugging configuration can broaden allowed origins; transport, authentication, process reachability, profile/target mapping and protocol compatibility need a separate ADR and security review. [Q2] Protocol `getCookies` and `setCookies` do not automatically provide a batch transaction or remote-state rollback either.

A future approved CDP backend must implement the same capability interface, have an explicit engine-version support matrix, and prove it does not expose ordinary user profiles through an unauthorised endpoint. If those conditions cannot be met, leave strict cookie features disabled. Core Versioned Browsing has no CDP dependency.

---

## 8. Command and action contract

### 8.1 One command system

Add definitions to the existing command and action registries. Use `parse_chain` for lexical handling. Convert its already parsed tokens into one closed typed `VersioningCommand` enum shared by UI, CLI and IPC. Reject unknown flags, duplicate mutually exclusive flags, trailing tokens and unsupported counts. Do not feed strings back through a second lexer or shell. [R12]

New action subjects `Commit`, `Stash`, `Branch` and `MergePlan` need complete handling in `as_str`, completion, availability and required-capability matches. Follow existing action naming patterns; proposed IDs below are new. Query commands must not accidentally create snapshots.

### 8.2 Normative initial grammar

| Command | Meaning |
|---|---|
| `commit MESSAGE [--scope=tab|window|context] [--pages] [--editable]` | Create immutable capture. Default scope window, structural only. `--editable` requires `--pages` and explicit sensitive-capture consent. |
| `commit-list [--limit=N]` | List normal-profile commits; no raw page text. |
| `commit-info REF` | Show coverage, sources, limits, objects and failure reasons. |
| `commit-delete REF` | Preview deletion, references and remaining retained copies; confirmation required. |
| `stash [NAME] [--scope=tab|window]` | Retain eligible live scope; default tab. No archive or cookie persistence implied. |
| `stash-list` / `stash-info ID` | List leases, state, age and approximate resource attribution. |
| `stash-resume ID` | Resume/move the retained instance into its original compatible host. Consumes stash ownership after successful adoption. |
| `stash-drop ID` | Explicitly release retained state with loss confirmation. |
| `branch NAME [--scope=tab|window] [--clean]` | Create new isolated child. Default tab. Without `--clean`, request verified snapshot inheritance; fail visibly when unavailable. |
| `branch-list` / `branch-info REF` | Lineage, profile mode, captured inheritance, retained parent and divergence. |
| `branch-delete REF` | Preview and discard a line, its transient profile and uncommitted live state. |
| `switch REF` | Focus/resume an existing line using exact owner identity. Never replace its profile. |
| `diff LEFT RIGHT [--kind=browser|page|cookies]` | Compare two explicit references; default browser. No `main..other` parser required. |
| `restore REF [--mode=live|archive] [--into=new-window|current] [--policies]` | Prepare restore; default live/new-window; archive needs captured document. Replacement and policy changes require preview/confirmation. |
| `merge BRANCH --cookies [--site=HOST]` | Create an immutable, expiring preview plan; performs no writes. |
| `merge-resolve PLAN ITEM main|branch|skip` | Select a whole supported cookie record for a conflict; does not edit a token value. |
| `merge-apply PLAN` | Native confirmation + revalidation + controlled application, only on supported backend. |
| `versioning-status` / `versioning-cancel OPERATION` | Inspect capabilities/jobs or cancel an admitted job. |

`REF` accepts an exact UUID, unambiguous UUID prefix of at least eight hex characters, or a unique name in the current authorised owner namespace. Ambiguity is an error. Messages may repeat; immutable IDs are authoritative. `main` is resolved within the captured lineage. Do not resolve by whichever tab happens to be focused when a callback runs.

`--scope=context` is enabled only after its slice is verified. A request for an unavailable option errors; it must not capture a different scope. Default snapshot branching is the **full-vision contract**, not a reason to block clean branching: help must explicitly offer `branch trial --clean` when snapshot capability is absent.

### 8.3 Confirmation, permissions and discoverability

Proposed action IDs include `browser.versioning.commit`, `browser.versioning.stash`, `browser.versioning.branch`, `browser.versioning.restore`, `browser.versioning.diff`, `browser.versioning.merge-preview` and `browser.versioning.merge-apply`. Resolve collisions in Phase 0 and document any naming adjustment.

Creating structural commits can follow the existing mutating command policy. Page capture, exporting transient contents, dropping only-live work, replacing a current window, applying old policies and writing cookies are sensitive. They require a captured native decision, never a page-supplied `confirmed=true`.

The existing `RequiredWhenChanged` confirmation type can be used with explicit application preview tokens. Extend it only if a tested requirement cannot be represented. Commands and scripts cannot replay an expired confirmation. Routine IPC can list redacted state and propose operations; sensitive apply requires the existing authorised native consent path. Do not expose raw cookie values over IPC.

All list/detail/preview controls use the universal switcher and one Versioning panel. No mandatory default keybinding that competes with current navigation. Users may bind commands through the existing configuration registry. Generated help and completion must include unavailable-state reasons.

---

## 9. Operation protocol and reducer integration

### 9.1 Common operation envelope

Every asynchronous operation has an opaque operation ID, owner token, captured subject set, configuration revision, privacy policy, expected logical revision, deadline and cancellation generation. The application coordinator admits the operation and emits a typed effect; Qt supplies a typed acknowledgement tagged with the same envelope.

Proposed states:

```text
Requested -> Validating -> AwaitingConsent -> Preparing
          -> Executing -> Verifying -> Publishing -> Completed

Any non-terminal state -> Cancelling -> Cancelled
Any state with no external mutation -> Failed
Any state with uncertain engine writes -> Partial / OutcomeUnknown
```

No success event is accepted twice. Late completions of cancelled operations clean up their own staging objects but do not resurrect UI state. A profile closed/replaced during work invalidates all attached permits. Deadlines are tested with a fake clock in core/application tests.

### 9.2 Typed integration

Proposed `Event::Versioning(VersioningEvent)` enters the reducer; `EngineEffect::Versioning(VersioningEngineEffect)` or an equivalent typed effect category leaves it. Add domain variants rather than stuffing JSON into a generic “execute arbitrary command” channel. Core algorithms receive time/UUIDs/capabilities as inputs and remain deterministic.

`BrowserApplication` owns the coordinator which combines runtime facts, effective config, storage tickets and opaque native handles. It must not mutate `ApplicationState` directly. The adapter translates native callbacks into events and executes already authorised effects.

When coordination crosses application/window owners, carry `ApplicationOwnerId` plus existing stable profile/window/tab/document identities. A small application-level session coordinator may route between registered `BrowserApplication` instances, but QML must not become the branch database or conflict resolver. No fallback to the primary window on an unresolved target.

### 9.3 Transaction boundaries

Structural snapshots freeze Ferric's logical inputs at a revision, not websites. A capture can finish for its originally admitted immutable input while the browser advances, provided no destructive replacement is implied. Page results require the original document token; if it changed, record unavailable rather than attach new-page data to the old page slot.

A group stash uses prepare/ack/commit: prepare fallback tabs and host leases, verify each requested freeze, then remove visibility membership. If a member fails before commitment, restore prior visibility and release only prepared leases. Do not silently stash half a requested window. After commit, later renderer loss is a separate lifecycle event with per-member reporting.

---

## 10. Durable storage design

### 10.1 Separate explicit capture from recovery

Do not enlarge `SessionSnapshot` schema 2 to include pages, secrets or branches. Introduce `VersionedCaptureManifest` schema 1 and deliberate conversion from existing safe tab descriptors. Recovery scheduling remains unchanged; committing is an explicit request. [R5, R10]

Store beneath the validated normal-profile state root:

```text
versioning/<profile-uuid>/
  objects/sha256/<two-hex>/<remaining-hex>
  staging/<operation-uuid>/
  quarantine/<operation-uuid>/
```

The existing profile SQLite store holds the commit index, immutable manifest bytes, branch references, object metadata and bounded journal. That store has one owning worker. Do not create a second SQLite writer connection inside the Qt adapter.

Recommended tables, with names adjusted only to avoid real schema collisions:

```text
vb_commits(id PK, profile_uuid, parent_id NULL, created_at, message,
           manifest_schema, manifest_bytes, manifest_hash, pinned)
vb_objects(hash PK, kind, bytes, state, created_at)
vb_commit_objects(commit_id, object_hash, role, logical_page_id NULL)
vb_refs(id PK, kind, name, commit_id NULL, updated_at)
vb_journal(sequence PK, time, operation_id, kind, safe_summary)
vb_operations(id PK, kind, state, safe_recovery_metadata)
```

Raw cookie values/bases/deltas are **not stored in these tables**. Branch live state is RAM-only. By default even metadata created inside transient/private owners stays transient; explicitly exported manifests become new normal-profile records only after the separate export path.

Choose the next migration number by reading the current migration system, not by assigning a guessed global version. All schema upgrades require rollback/reopen and corruption tests.

### 10.2 Manifest contents

```text
VersionedCaptureManifest v1
  id, profile_uuid, created_at, message, parent_commit
  application_revision, configuration_revision
  scope, requested_capture, capture_interval
  windows[]: logical_window_id, context_ref?, selected_logical_page, pages[]
  pages[]: logical_page_id, lineage_parent?, safe_url_or_placeholder,
           title_redacted, pinned, muted, zoom_hundredths,
           document_scroll?, archive_object?, sidecar_object?, fidelity
  policy_snapshot?: allowlisted declarative values + schema hash
  engine_fingerprint, helper_bundle_version
  warnings[]: stable reason codes
```

Use integer zoom hundredths and validated finite numeric positions; reject NaN/infinity, negative sizes and excessive nesting. Store only canonical safe URL descriptors unless explicit page capture authorises a sensitive payload. The archive may still contain full URLs: its sensitivity is not erased by safe metadata.

### 10.3 Publish protocol

1. Worker admits a bounded staging job under an exact, exclusive operation directory and enforces disk budget.
2. Qt writes only the authorised capture path. Native capture completion is not yet a committed object.
3. Bounded storage I/O validates file type/size, reads incrementally, hashes exact bytes and fsyncs the object. Use a reviewed hash implementation; do not copy build-script hashing code into a new security-sensitive library without review.
4. Rename into immutable object location and fsync its directory. An existing same-hash object must match validated size/content rules before reuse.
5. In one SQLite transaction, insert the immutable manifest, object references and successful operation record; commit using the existing durability settings.
6. Only then announce the new commit and clear staging.

A crash before step 5 leaves an orphan, not a visible commit. Recovery can quarantine or garbage-collect the orphan. A database reference to a missing/corrupt object makes that page unavailable; do not substitute a live URL while calling it an archive.

A bounded blob-I/O worker may handle hashing and copy work so large captures do not starve metadata. It cannot access SQLite; the existing profile metadata worker remains authoritative. Maximum two blob jobs, one page-save admission per profile, cooperative cancellation between chunks. Do not increase existing queues without measured need.

### 10.4 Retention and deletion

Named/user-pinned commits are never automatically deleted to satisfy quota. Automatic journal entries and unreferenced staging/orphans are bounded. GC is mark-and-sweep from commits, pins, active operations and explicit durable references; mark under a storage revision and revalidate before sweep. A preview reports which data remains referenced elsewhere.

Dropping a commit is not a claim of forensic secure erasure: SQLite/WAL backups, filesystem snapshots and other captures may retain copies. History clearing and versioned capture deletion are distinct scopes, with a clear user choice to include both. Do not let routine history cleanup silently destroy named commits or falsely imply it removed archived content.

---

## 11. Structural commit, diff and restore

### 11.1 Structural capture

Resolve scope to an ordered set of captured owners. Project existing tab state plus validated scroll values, context references and supported configuration data. Preserve safe placeholders for POST/unsafe URLs instead of resubmission. Borrow the existing safe descriptor rules; do not loosen them through the new command. No UI thread filesystem I/O.

A structural commit records the state at the admitted Ferric revision. Later tab changes do not invalidate this immutable structural input. New schema fields must represent real existing data or an explicit unavailable capability. In particular, do not invent tab tags, resource pools, compositor layouts or historical website back/forward stacks.

### 11.2 Semantic diff

Compare logical identities, not new runtime IDs. Report additions/removals, ordering, selected page, URLs, pinned/muted/zoom/scroll values and allowlisted policy changes. Page diff reports capture availability and supported categories changed; it does not print a DOM patch or private form contents.

Treat ordering deterministically. A removed page and a newly opened equal URL are not automatically the same logical page. Treat unknown/unavailable as unknown, not an empty value. Output is paginated and bounded; JSON projections contain stable reason codes.

### 11.3 Restore plan

Default restore opens a **new normal-profile window** associated with the commit's validated owner profile. It allocates new runtime IDs and maintains a logical-to-runtime mapping. Restore selected tab first, other URL loads lazily. The current workspace is left intact.

`--into=current` previews replaced pages, preserves a structural recovery checkpoint, checks unsaved work and requires confirmation. A structural checkpoint cannot preserve unknown unsaved web application state; offer a live stash or cancel rather than saying replacement is safely reversible.

Implementation boundary (2026-10-07): VB-024 covers only the default new-window route, for which the source remains intact. The independently gated `--into=current` path is VB-096. An unavailable qualified stash must produce refusal without mutation; completion of the new-window route does not imply replacement is safe.

Policies are descriptive by default. `--policies` previews an allowlisted configuration diff and applies it through current validation. Never restore permission grants, trust exceptions, external command paths, passwords or weakened security settings automatically. Record and respect the current policy revision on live loads.

A requested lifecycle `Frozen` in a stored manifest is historical metadata, not proof that a newly restored page is frozen or live-retained. Load, check eligibility and observe the real engine state before applying a lifecycle policy.

---

## 12. Live stash and retention implementation

### 12.1 Admission and preservation intent

`stash` captures a user request to retain work, not permission to destroy it. Resolve the target while it is still visible. Prepare a replacement blank tab or compatible host before hiding it. Acquire a profile lease and a view lease, then request eligible lifecycle transition.

Admission checks include renderer health, valid owner/document, no in-flight navigation or download/prompt/capture operation tied to the view, no attached DevTools, and qualified lifecycle eligibility. Existing ordinary suspension has additional editable-state guards. A new `PreservationIntent::ExplicitStash` may permit dirty editable content only after native confirmation and a dedicated dirty-editor qualification test. It must not imply that the data has been saved. [R9]

If Qt will not safely freeze a view, report the reason and leave it available. An optional explicit `RetainActive` choice may keep a hidden live page running, with a visible CPU/network warning and `live-active` status. Do not present it as frozen. The initial command can simply refuse such cases until that optional behaviour is qualified.

Never move a camera/microphone/WebRTC call into an apparently dormant stash. Stop/leave the call through the application's ordinary user action or refuse. Do not automatically revoke media permissions as a substitute for correctly handling the activity.

### 12.2 Qt view ownership

The adapter's view registry maps a `RetainedViewId` to the actual view plus its owner profile lease and original host metadata. A native parent or explicit owning component must survive tab-list reconciliation and window close. A JS reference alone does not override QObject parent destruction.

The existing view-pool reconciliation destroys unmatched views. Introduce a typed membership projection: `Visible`, `Retained`, `Transferring`, `Closing`. The pool must leave `Retained` entries with their retaining host rather than treating them as stale. Keep the current same-profile live transfer prepare/adopt/rollback pattern. [R13]

The smallest qualified implementation retains views within their original host's invisible retention container and keeps that host alive while leases exist. Closing its visible window requests an explicit decision: keep the hidden owner, resume elsewhere through the qualified same-profile transfer path, or release the stash. Application shutdown always acknowledges volatile loss.

After that works, a long-lived per-profile retention host can support ownership independent of a particular visible window. This is a separate slice; do not mix it with initial freeze semantics. Never reparent into a view that changes the profile.

### 12.3 Resume and consumption

On resume, locate the exact lease, verify that the QObject and renderer still exist and the original profile instance matches. Prepare the destination slot before releasing stash ownership. Set Active as needed for visibility, adopt, observe readiness, then remove the stash record. A failed adoption returns ownership to the retaining host without destroying the page.

Resume consumes the stash lease, not the page. There is no second immutable copy left behind. A future stash of that page is a new retention operation. `stash-info` must make this clear.

### 12.4 Renderer loss and shutdown

`renderProcessTerminated` marks every affected retained record `lost`; use actual owning view signals rather than assuming one PID belongs to one tab. [Q1, Q8] Do not automatically reload a lost stash and call it restored. Offer a separately captured durable fallback if one exists, with the weaker reconstruction label, or a safe URL placeholder.

Shutdown freezes no new work. It cancels pending versioning jobs, previews volatile losses, and then destroys retained views before releasing final profiles. OS kill and process crash can bypass UI confirmation; the feature cannot guarantee live recovery in those cases. A normal-profile safe descriptor may survive only through existing explicit recovery policy, never by copying the stash heap or its secrets to disk.

---

## 13. Durable document capture

### 13.1 Representation

Use MHTML (`MimeHtmlSaveFormat`) as the first archive backend. It is a stored document representation, not a runnable replica of the original authenticated application. Record format and backend version in the manifest. CompleteHtml is a possible later backend but not required; it adds a resource directory and more path-validation complexity. [Q5]

Archive capture is opt-in per operation. Before first use, disclose that page archives may include personal data, embedded secrets and resource URLs. Do not call the archive sanitised unless a separate verified sanitisation pipeline exists. A redacted title or sidecar does not sanitise the archive.

### 13.2 Capture job

Proposed job states:

```text
Admitted -> StagePrepared -> DownloadCorrelated -> Saving
         -> NativeCompleted -> BlobValidated -> ManifestPublished
         -> CleanupComplete
```

The worker creates a restrictive, unpredictable operation directory first. The application then authorises exactly one save for the captured view/profile/path. Invoke `view.save` only after that permit is installed. Page-save downloads auto-start, so the safety check cannot rely on an ordinary “ask where to save?” dialog after the call. [Q5]

At `downloadRequested`, use profile instance, active capture permit, exact expected path and public download facts to correlate the request. Validate the actual QML download surface on the baseline; do not reach through a private `QWebEnginePage*`. If a public per-view match cannot be obtained, serialize save requests per profile, match the preauthorised path and fail closed on ambiguity. Ordinary concurrent downloads must still use their existing route. A page-triggered unrelated download cannot steal the capture permit.

Handle `DownloadCompleted`, cancelled, interrupted, profile close, disk full, deadline, navigation and empty output separately. `isFinished=true` alone is not success. Connect only once and disconnect on retirement. Native `cancel` is not a substitute for application cleanup verification.

The backend may fetch resources during saving. Capture is not guaranteed network-free or instantaneous. Keep normal profile/network policy in force. Do not bypass authentication, CSP, blocked resource policy or certificate handling to improve fidelity.

### 13.3 Consistency of page capture

Capture an interval: start monotonic time, end monotonic time, initial and final owner/document tokens, plus helper mutation counters when available. A navigation/replacement invalidates that page's capture. Same-document changes may yield a partial capture with an interval warning; a DOM mutation counter is useful evidence but not a global snapshot barrier.

Do not freeze a page before scheduling a script and then wait forever for the script to run. Probe/capture while active, then hide and freeze for the stash/branch workflow if appropriate. This ordering must be exercised with real timers, fetches and page lifecycle callbacks. Default capture does not freeze ordinary browsing merely to produce an archive.

For a window capture, publish all requested page results as explicit success/failure entries. The commit itself can be valid with partial coverage when the preview/command policy permitted partial capture. A strict capture option, if later added, must leave no visible partial commit on failure.

---

## 14. Bounded semantic page state

### 14.1 Collector request and result

The collector is a reviewed, versioned browser script executing in ApplicationWorld. It receives a closed request with a nonce, requested categories and limits, not arbitrary JS from a stored manifest. It returns plain data only. It never receives file paths, cookie values, native commands or profile handles. [Q3, Q4]

Proposed request categories:

```text
geometry:    top scroll, eligible nested scroll offsets, zoom context
controls:    details/open state and selected options
editable:    permitted text/textarea/checkbox/radio values (explicit opt-in)
selection:   bounded text selection positions (opt-in)
media:       currentTime and paused metadata (never autoplay)
```

Use bounded traversal that stops after a fixed number of nodes; do not allocate `querySelectorAll('*')` over an unbounded document. Check elapsed time inside loops and return `truncated` when limits are reached. Limit JSON bytes, strings, result depth and per-frame counts again at the Rust boundary. Timeout abandons the operation; it cannot guarantee preemption of a malicious renderer.

`PageStateSidecar` includes schema/helper version, capture fingerprint, frame descriptors, field descriptors, numeric positions, requested categories, coverage and exclusions. Editable values are not included by default. Sensitive records must not implement an unredacted Debug formatter.

### 14.2 Element descriptors are candidates, not authority

For a new live document, stored descriptors can help find candidates using stable id/name/type/label structure. Require exact origin/route qualification, a unique match, expected element type and bounded context checks. Ambiguous or changed candidates are skipped. Do not blindly replay a CSS selector against a different document or choose the first match.

This does not change ADR-0014. Its selected scroll-target policy is never persisted. On a reconstructed document, explicit scroll target begins `auto`; applying a numeric offset to a newly validated candidate does not make that candidate the keyboard scroll owner. [R11]

### 14.3 Frames and shadow trees

Start with the top document and reachable same-origin descendants. Qt's public frame API can independently address frame objects, but it does not justify collecting arbitrary embedded identity-provider state by default. A later opt-in per-frame scope requires exact origin authorisation and callback validation. Children are unordered, so child array index is not a durable identity. [Q4]

A missing, navigated, cross-origin-unapproved, opaque-origin or sandbox-restricted frame is an exclusion, not an empty frame. Open shadow roots may be inspected in a bounded way when qualified; closed shadow roots and custom application state are not generic restore targets. Captured resource pixels/canvas are not necessarily persisted by a DOM sidecar.

### 14.4 Sensitive exclusions

Never collect password values, file input paths/blobs, hidden authentication controls, WebAuthn state, permission grants or cookies through page JS. Exclude common token/OTP/autocomplete-sensitive fields and report exclusions. These heuristics reduce exposure; they cannot prove that all secrets were removed from ordinary text or the archive. A document-level opt-out list can refuse capture entirely.

Contenteditable restore starts as a recoverable text draft shown to the user, not arbitrary HTML reinjection into a live remote editor. Application-specific structured editors need a separate, reviewed adapter with tests; they are not implied by generic contenteditable support.

### 14.5 Live reconstruction

Load a validated safe URL into the chosen current profile or isolated branch. Wait for a bounded readiness window. Revalidate document and origin after redirects. Install only reviewed helpers; never execute captured page scripts to recreate the heap.

Apply geometry and benign presentation state only where validated. Editable refill has its own confirmation and is conservative: set approved native control properties, do not synthesize input/change/submit events, and never submit. Focus restoration can trigger page handlers; keep it optional. Even property changes or navigation can cause site effects, so the product must not guarantee that live reconstruction is remote-side-effect-free.

React/Vue/other application state may reject or overwrite DOM properties. Verify the resulting values after a bounded interval and report restored/skipped/overwritten. Do not fight the application with an indefinite retry loop. A refused editable restore should offer a local draft view rather than attempt increasingly invasive scripts.

Media position may be displayed or restored only under current media policy; never resume playback, device capture or a call automatically. Navigational history remains a fresh live context; do not replay History API scripts or POST requests.

---

## 15. Historical archive viewing

### 15.1 Security boundary

An archive is untrusted web content, even when generated by Ferric. Open it in a new dedicated off-the-record archive profile, not the user's normal profile or an authenticated experimental branch. Configure **before navigation**:

- JavaScript disabled for archive content;
- no page/native bridge or ordinary userscripts;
- external network requests denied;
- remote/local file access restricted to the authorised archive representation;
- downloads, popups, external protocols, clipboard writes, device permissions and form submissions denied;
- source URL shown as metadata, not a claim that the archive has that site's live authority.

Use the existing public request-interceptor integration and a reviewed file/custom-scheme load path. Do not insert archive HTML into a privileged Ferric internal page. Test MHTML subresource resolution under these controls; failure to prove isolation disables archive viewing, not the network guard. [Q5, Q9]

An interceptor alone is not a universal proof against every networking channel. Qualification must observe zero unsolicited traffic from malicious archive fixtures, including redirects, CSS resources, nested frames and external navigation attempts. Fresh profile + disabled scripts + no workers/popups substantially narrows the surface, but the guarantee still requires runtime tests.

### 15.2 Live opening is explicit

The archive surface has a separately authorised “Open live URL” action. It creates/navigates a normal target through current URL policy, using the current chosen identity. It does not turn the archive into a live authenticated document in place. The user must see the transition between “captured” and “live”.

---

## 16. Isolated branch creation and switching

### 16.1 Profile isolation model

Every child line has a distinct `QQuickWebEngineProfile` instance. Creating several views with `defaultProfile()` does not create isolation. Reuse current prototype configuration with explicit memory-only cookie/cache choices, current security policy and no durable storage path for the branch. [Q6, Q8]

Initial child windows are ephemeral at the engine/storage layer. Do not attach them to a durable context or parent `ProfileStore`. A separate lineage record in the parent is allowed to record the explicit creation action, but child browsing history/cookie changes do not leak automatically into that parent record.

One branch profile may host several tabs/popups in that branch. Inherit no parent permission grants, certificate exceptions or external action authority. Popups remain in the originating branch profile; opener handling must not cause a normal-profile fallback. Cross-profile `window.opener` continuity is not preserved by reconstructing a new child.

### 16.2 Clean branch transaction

1. Resolve the source and capture its structural state plus optional supported in-memory sidecar.
2. Prepare a new branch record, owner token, child host and fresh OTR profile without navigating.
3. Apply current network/permission policy to the new profile and register ownership.
4. For the initial clean mode, leave the parent view visible and active in its original owner. A later hidden-parent mode may prepare retention and hide/freeze eligible pages only after destination/fallback allocation succeeds and VB-098 qualifies the exact live leases.
5. Navigate the child to safe URLs, reconcile approved state, and publish actual inheritance coverage.
6. Mark the child active and expose the line through switcher/list UI.

If steps 1–4 fail, the source remains usable and the prepared child is torn down. If navigation fails, keep an inspectable failed child with a safe placeholder and a working parent; do not delete the parent's retained page. A POST-only or unsafe source can branch into a blank placeholder or require the user to choose a safe URL, never replay the request.

The visible-parent mode does not promise a frozen source or an immutable external environment: the parent can continue executing scripts, network requests, downloads and remote writes. UI/status must say `parent-active`. It does provide a distinct child engine profile with no local cookie/storage inheritance. Hidden-parent retention, freeze and recovery remain separately gated by VB-033/VB-036/VB-098.

For window branches, all children use the one new branch profile. Scope completion is group-consistent for Ferric ownership; individual live site reconstruction may be partial. Parent retention must be reported per page.

### 16.3 Snapshot-seeded branch transaction

The full-vision default adds a cookie snapshot gate between preparation and child navigation. Requirements are:

- supported complete cookie identity for the requested scope;
- a reliable source snapshot boundary and completeness evidence;
- eligible attributes and expiration handling;
- a destination with no untrusted navigation before seeding;
- seed completion/read-back verification.

Without these, return `CookieSnapshotUnavailable` with the capability reason. Offer a separate clean-branch command; never silently copy a guessed subset while showing “snapshot-isolated”.

When supported, the branch eagerly materialises a private copy of the source jar. There is no live fallback to the parent's jar. Expiration continues according to real time. Subsequent mutations are local to the branch's engine profile. This is snapshot-isolation behaviour for supported cookies, not literal Chromium overlay storage.

### 16.4 Parent and child interaction

Other tabs using the parent's original profile may change cookies, local storage or servers while a parent view is retained. A branch protects against its own local writes contaminating the parent jar, but it does not provide a globally immutable parent environment.

Switching line does not restore old parent cookies. A resumed parent uses its actual profile state at resume time. Cookie merge is an explicit, separate operation. This separation avoids hidden authentication rollback merely when focusing a tab.

### 16.5 Exporting transient branch work

Keep ordinary private/ephemeral durability rules. To preserve child work across restart, an explicit branch-origin `commit` requests export of structural/page data to its linked normal owner, with a native preview of destination and sensitivity. A new ADR must authorise this narrow path. No cookie values or opaque profile databases are exported.

A generic private browsing window without a normal owner is not allowed to smuggle data into a default normal profile. Explicit file downloads remain the user's ordinary escape route under existing policy. Automatic child checkpoints, journals and history remain RAM-only.

After restart, an exported child commit is a durable captured representation, not a resurrected authenticated branch profile. Reopening creates a clean line or requires fresh supported inheritance. Never silently use the parent's current cookie jar as the historical child.

---

## 17. Cookie backend and observation model

### 17.1 Interface before implementation

Use a backend contract that can truthfully express absence of guarantees:

```text
CookieBackendCapabilities
  identity: lossless(scope) | lossy(reason)
  snapshot: consistent(scope) | observed_only | unavailable
  read_back: exact(scope) | observed_only | unavailable
  writes: individual_async | verified_batch | unavailable
  exclusive_apply: supported(scope) | unavailable
  supported_attributes, engine_fingerprint, evidence_ids

CookieReadResult
  records, coverage, consistency, read_interval, revision_token?
```

The public Qt adapter initially reports an **observed-only, potentially lossy** view for general profiles. Do not set `lossless` because a small fixture with ordinary cookies passed. Tests must exercise the requested real cookie classes, and source/API review must explain where each required attribute comes from.

An application-owned fresh fixture profile can establish a known initial empty state for algorithm testing. That is not proof of arbitrary persistent parent completeness or of all future mutations being representable.

### 17.2 Cookie descriptors

Preserve raw name/value bytes. Do not Unicode-normalise or log opaque values. Proposed descriptor:

```text
CookieKey
  backend identity version
  name bytes, canonical host/domain, path
  host_only flag with known provenance
  partition identity (explicit unpartitioned, full known key, or unknown)
  additional backend identity dimensions, when required

CookieRecord
  key, opaque value, expiration/session marker
  secure, http_only, same_site (including unspecified)
  supported additional attributes with explicit presence
  fidelity/provenance
```

Do not permanently define a universal Chromium identity as only name/domain/path. The exact identity and attribute requirements belong to the selected engine/backend contract, and unknown identity cannot be merged. Read values may be kept in a dedicated memory-only secret container; ordinary projections use opaque handles and change indicators.

The Qt adapter must preserve host-only scope correctly. Passing a non-dot domain to `setCookie` can broaden scope; a qualified host-only conversion clears domain and supplies the correct origin. Do not invent an origin from an arbitrary currently focused URL. [Q6]

### 17.3 Observation pipeline

Connect observations before profile initialisation where possible. Queue minimal copied typed facts, process them outside the IO callback, and bound queue size. Each profile instance gets an observation epoch and monotonically increasing local sequence. Queue overflow, profile replacement, identity collision or malformed/unknown data invalidates completeness and all pending plans.

`loadAllCookies` emits added notifications for enumerated cookies. Distinguish rescan observations from known live changes when the backend can do so; otherwise call the stream an observation log, not an authoritative mutation journal. Repeated enumeration must not manufacture “branch-created” history.

An empty observed map after a timeout is **unknown**, not an empty Chromium jar. Deletion absence from incomplete enumeration is not a tombstone. The implementation must have tests specifically preventing those two mistakes.

### 17.4 Sensitive-state retention

Cookie bases, overlays and merge values stay in memory. Keep only the current base and necessary deltas, not an unbounded history of expired authentication tokens. Destroy them when the final branch/plan owner closes. Where Rust secret wrappers can erase buffers on drop, use a reviewed implementation; no claim of eliminating Qt/Chromium copies, swap or OS crash dumps.

Do not put raw cookies in SQLite, archive sidecars, normal logs, exception text, test snapshots or content-addressed public objects. A future durable identity vault is outside this proposal's enabled scope and requires separate key-management/security design.

### 17.5 Attribute and authentication limits

HttpOnly access through the native store is technically possible, but it does not make cookies safe to export. Session, Secure and HttpOnly flags do not reliably identify authentication: any cookie may be credential-bearing. Treat all values as secrets.

A cookie-only seed can fail on applications depending on localStorage, IndexedDB, device-bound sessions, service workers or server-side checks. It may also be accepted as the same remote session as its parent. Report “cookie state copied” rather than “identity cloned” or “account fully isolated”.

Default intended snapshot scope is the parent profile to support multi-origin authentication, but that exposes a wider set of credentials to the child. Require an explicit preview of scope and consent. Do not infer all SSO domains from the visible tab. Site-scoped copying can be a later expert option with an explicit incomplete-authentication warning; it is not a substitute for full-scope fidelity.

---

## 18. Three-way cookie merge

### 18.1 Pure planner

The planner operates on immutable, complete snapshots for a supported scope. Its inputs are base B, current parent M and current child C, plus a single evaluation time. It has no Qt, filesystem, clock lookup or network dependency.

For every key in the union:

```text
if C == B: keep M
else if M == B: propose C
else if M == C: converged; no write
else: conflict
```

Equality compares the entire supported semantic record, not just value bytes. Absence is a first-class state only when snapshot completeness is known. Unknown yields an unavailable item, never deletion.

| Base | Parent | Child | Plan |
|---|---|---|---|
| A | A | B | Propose child B |
| A | B | A | Keep parent B |
| A | B | B | Converged |
| A | B | C | Conflict |
| A | A | absent | Propose deletion |
| A | absent | A | Keep deletion |
| A | absent | B | Conflict |
| absent | absent | B | Propose addition |
| absent | B | B | Converged addition |
| absent | B | C | Conflict |
| A | absent | absent | Converged deletion |

Expiry is evaluated against one injected clock. Never extend a stored expiration automatically. Expired records are not resurrected. A disappearance caused by expiry or unknown observation must not be mislabelled as an intentional branch logout. Distinguish observed delete intent from effective absence in diagnostic explanations; merge decisions still require known snapshots.

### 18.2 Plan and preview model

```text
MergePlan
  id, parent/child exact profile instances, lineage/base reference
  parent/child backend revision or snapshot evidence
  evaluation time, expiration deadline, config/security revision
  supported scope and coverage report
  items[]: key handle, reason, proposed operation, conflict choice
  expected before/after records (secret store, not UI)
  confirmation digest over immutable plan + choices
```

Preview shows host/path/name only under normal privacy policy, changed metadata, addition/deletion/conflict counts and general authentication risk. Escape all strings as untrusted UI text. Raw values are never shown by default and need not have a reveal feature in this proposal.

Per-cookie merging can create an application-invalid combination even when every three-way comparison is conflict-free. Ferric cannot discover all session/CSRF/token coupling. Group related host changes in the preview, warn that selecting a subset may break authentication, and do not claim semantic account merge. `--site` is a scoped filter with precise host/domain rules, not a promise that one origin owns every related cookie.

### 18.3 Preconditions for apply

The planner is implementable now. **Applying to a normal parent profile is a separate, stronger capability.** It requires reliable current read-back, exact supported identity, deterministic mutation acknowledgement/verification, and a demonstrated way to handle concurrent engine mutation for that scope.

A revision check followed by asynchronous writes is vulnerable to a race after the check. Freezing tab views does not stop all workers, in-flight responses or network service cookie updates. An interceptor is not a global cookie transaction lock. If no qualified backend can establish sufficient control, `merge-apply` remains unavailable for live normal profiles.

Do not expose a misleading “transactional” mode just because Ferric stores a pre-merge snapshot. An experimental best-effort cookie copier would be a different product decision and is not this command's default semantics.

### 18.4 Controlled application when prerequisites pass

1. Validate plan ownership, coverage, expiry, confirmation and current security policy.
2. Acquire the backend's qualified write session for the scoped profile. Re-read parent and child; reject stale plans before any writes.
3. Retain a bounded in-memory preimage only for touched keys.
4. Apply a deterministic ordered operation sequence through the real cookie store; record each acknowledgement and verified read-back.
5. Stop on unexpected writes, timeout, unsupported key, profile loss or verification mismatch. Never continue blindly through the rest of a credential batch.
6. Verify the resulting touched scope. Only then publish `Completed` and a redacted merge receipt.
7. On partial failure, attempt compensation only where the backend can prove the current record is still the exact value written by this operation. Otherwise leave the profile in an explicit `Partial/OutcomeUnknown` state and stop dependent operations.

Even successful per-key verification is not evidence of an atomic multi-cookie commit. User-visible wording must say “verified changes applied” unless the backend genuinely supplies stronger semantics. Do not log the preimage or write a cookie rollback file.

### 18.5 Repeated and partial merges

Do not overwrite the entire fork base after a partial merge. Keep per-key reconciliation anchors/receipts for successfully applied or already-converged items; untouched/conflicted keys retain their prior base. A later preview compares against the relevant anchor, avoiding false conflicts caused merely by an earlier successful merge.

`skip` means not reconciled and advances no anchor. Choosing parent while child differs does not silently rewrite the child profile. Record the decision for that plan; reappearance in a later plan is intentional unless an explicit future synchronisation operation changes the child. Never infer that merging cookies should also merge the child's page state, permissions or storage databases.

---

## 19. Other state classes and explicit exclusions

| State | Capture/retain | Seed/reconstruct | Merge | Contract |
|---|---|---|---|---|
| Browser structure | Durable typed capture | Supported | Selective restore, not arbitrary merge | Core feature |
| Current live DOM/JS | Retain actual view | Not cloneable | None | Live stash only |
| Document archive | Explicit durable file | Offline historical view | None | Capture backend qualification |
| Bounded fields/scroll | Opt-in sidecar | Validated candidates, partial coverage | No general page merge | Conservative reconstruction |
| Cookies | Native observed mirror; strict snapshot gated | Strict seed gated | Three-way planner; apply separately gated | Conditional advanced feature |
| `sessionStorage` | Origin-scoped API exists | Future reviewed adapter | None | Excluded from default inheritance |
| `localStorage` | Origin-scoped API exists | Future reviewed adapter | None | Excluded; can contain credentials |
| IndexedDB | Browser-owned structured store | No generic clone promise | None | Non-goal for this implementation |
| Cache Storage/service workers | Browser-owned storage/execution | No generic clone promise | None | Non-goal |
| Permissions | Inspect current policy only | Fresh/current prompts | None | Never inherit old grants |
| HTTP auth/client certificates | Native/browser/system state | User reauth; no clone guarantee | None | Do not duplicate automatically |
| HSTS/DNS/network caches | Engine/network state | Not a branch snapshot contract | None | No anonymity claim |
| WebAuthn/passkeys/device binding | External/system and origin constraints | Normal user flow only | None | Explicit non-goal |
| Push subscriptions | Service/engine/server state | Not inherited | None | No background identity cloning |
| Remote data and actions | Outside Ferric ownership | Current server state | None | Never reversible by local restore |

Separate OTR profiles are a browser storage boundary, not a VM or OS privacy boundary. They do not create distinct IP addresses, machines, hardware authenticators or remote accounts. Process/GPU resources may be shared; account/session servers remain external. [Q8]

---

## 20. Security, privacy and data handling

### 20.1 Local threat model

This feature protects against accidental state loss, accidental local-profile contamination and untrusted web content obtaining native authority. It does not protect against a malicious process running as the same OS user or an administrator.

Create versioning directories with mode 0700 and files with mode 0600, using safe no-follow/path containment primitives where available. Validate owners and avoid symlink traversal/overwrite. Reject an existing untrusted path rather than “repairing” permissions while following it. No filenames derived directly from page titles, URLs or cookie names.

The initial storage protection is **restricted local files**, not a new encryption vault. The UI must say that archived page contents may remain readable to software running as the user and may enter system backups. Encrypted-at-rest blob support can be a later reviewed backend; do not invent home-grown cryptography or silently store keys beside supposedly protected secrets. This limitation does not permit persisting cookie jars: those remain memory-only.

### 20.2 Capture versus export

An explicit `--pages` request is analogous to deliberately saving a page, with stronger organisation and consent. Automatic checkpoints never save pages. `--editable` is an additional request; it does not imply all fields are safe. Private captures remain memory-only/no durable export in this feature. Branch exports use the narrow normal-owner path and preview defined in Section 16.5.

### 20.3 Archive and sidecar trust

Treat every captured title, URL, HTML part, selector candidate and field value as untrusted. Stored data cannot enable scripts, choose executable paths, alter engine flags or request native privilege. Confirmations come from native UI with an immutable captured target, not from the page.

All rendering surfaces escape untrusted labels. IPC omits page text and cookie data. Testing fixtures use synthetic secrets and assert they never appear in logs or protocol snapshots. A debug build is not exempt from secret handling.

### 20.4 Post-merge user guidance

Merging may change login state, invalidate open pages, create inconsistent application state or affect the same server session used by the parent. Do not silently reload all matching pages after merge; a reload can lose unsaved work or trigger remote requests. Present affected profiles/origins and an explicit reload choice under normal unsaved-work guards.

A cookie rollback cannot revoke a remote action or restore a revoked server token. Browser-held history is not a general undo for web applications.

---

## 21. Resource limits and operational defaults

The following are proposed starting limits. Phase 8 measurements may adjust them with documented evidence, not silently remove boundedness.

| Resource | Initial default |
|---|---|
| Versioning operations | One mutating operation per application owner; global bounded queue of 16 |
| Page saves | One admitted save per profile; at most two across the process |
| Retained live views | 8 total by default; configurable to 32 with warning |
| Simultaneous branch profiles | 4 by default; configurable to 16 |
| Branch nesting | 8 |
| Pages per explicit window capture | 128; reject/confirm narrowed scope rather than silently omit |
| Frames per sidecar | 32; depth 8 |
| DOM nodes visited per frame | 4,096 with an elapsed-time check |
| Captured controls per page | 256 |
| Editable value | 16 KiB per value; 256 KiB total per page |
| Entire sidecar | 512 KiB validated JSON; depth 16 |
| Archive object | 64 MiB nominal admission cap; handle native overshoot explicitly |
| Versioning disk budget | 2 GiB per normal profile; no automatic deletion of named commits |
| Journal | 10,000 redacted events; no cookie values |
| Cookie observations | Bounded 4,096-event queue; overflow invalidates mirror |
| Cookie material | 16 MiB aggregate per live branch; fail, do not truncate silently |
| Listing | 100 items default, 1,000 maximum; pagination |
| Confirmation plan | 60-second validity; always revalidate on apply |
| Collector callback deadline | 2 seconds |
| Archive save deadline | 60 seconds, explicit failure rather than perpetual waiting |
| Live restore readiness | 15 seconds; one bounded reconciliation pass plus verification |

Large native page saves may already have written beyond a quota before a queued callback runs. Use admission reservation, monitor bytes, cancel on limit and clean/quarantine leftovers; do not claim an exact hard write cap unless OS-level enforcement is separately implemented.

Memory accounting must avoid claiming each tab's renderer PID is exclusive. Show known view/profile counts and measured process-level memory, or explicitly labelled estimates. Do not sum a shared renderer's RSS once per tab. Release retained resources only through explicit policy; never silently downgrade the only live copy to a URL descriptor.

Suggested configuration additions, all proposed:

```toml
[versioning]
enabled = false                  # opt in during rollout
max_retained_views = 8
max_branch_profiles = 4
disk_budget_mib = 2048
page_capture = "ask"
editable_capture = "ask"
cookie_snapshot = "qualified-only"
cookie_merge = "qualified-only"
```

Validate these through current config/settings registries. A configuration value cannot force an unsupported capability to become true. Existing resource policies continue to apply; no separate scheduler competes with them.

---

## 22. Error and recovery semantics

Use the existing IPC error envelope and extend its typed vocabulary only where needed. Proposed stable versioning reason codes include:

`VersioningDisabled`, `UnsupportedCapability`, `UnqualifiedRuntime`, `StaleOwner`, `DocumentChanged`, `WrongProfile`, `PrivatePersistenceDenied`, `ConfirmationRequired`, `ConfirmationExpired`, `AmbiguousReference`, `Busy`, `QuotaExceeded`, `CaptureTimeout`, `CaptureInterrupted`, `UnsafeRestoreDescriptor`, `ArchiveIsolationUnavailable`, `RetentionUnavailable`, `RetainedRendererLost`, `CookieIdentityLossy`, `CookieSnapshotIncomplete`, `CookieSeedUnverified`, `MergeStale`, `MergeConflict`, `MergeApplyUnavailable`, `MergePartial`, `MergeOutcomeUnknown`, `ObjectCorrupt`.

Map these into existing high-level error codes with redacted details. Do not add several overlapping error formats. Query operations return capability/fidelity facts without raising a generic internal error for expected unsupported behaviour.

| Failure | Required outcome |
|---|---|
| Page navigates during capture | That page result unavailable; no mixed-document sidecar |
| Archive saving fails | No successful archive reference; stage cleaned/quarantined |
| Store is full | No visible incomplete commit; existing commits intact |
| UI closes during save | Operation cancelled by owner; late callback cannot publish elsewhere |
| Parent stash cannot freeze | Keep source usable; no false frozen status |
| Retained renderer crashes | Mark lost; explicitly offer weaker fallback if available |
| Child fails to reconstruct | Parent remains available; child shows partial/failed state |
| OTR profile allocation fails | Roll back prepared branch ownership, not the source page |
| Cookie observation overflows | Invalidate strict snapshot/merge plans |
| Cookie apply outcome unknown | Stop, mark partial/unknown, do not blindly overwrite main |
| Commit object corrupt | Report unavailable historical object; no quiet live substitution |
| Missing crypto/key backend in a future extension | No plaintext downgrade presented as encryption |

Every operation reaches a terminal state or a clearly bounded awaiting-user state. Background cleanup is performed by ordinary running application work, not by leaving orphan promises or hidden UI locks.

---

## 23. Validation and qualification strategy

### 23.1 Test layers

**Core:** pure unit/property tests for IDs, lineage, references, scope, validation, projections, three-way truth table, expiry, unknown states, cancellation, redaction and resource bounds. No Qt, display, filesystem or network.

**Application:** deterministic fake engine/storage/clock. Inject callback reorder, duplicate acknowledgement, owner loss, stale confirmation, partial group failure and unsupported capabilities. Fakes test orchestration, never qualify native behaviour.

**Storage:** disposable roots and real SQLite; migration reopen, power-loss cut points, disk-full simulation, symlink attacks, hash mismatch, orphan recovery, reference integrity and bounded GC. Ensure current schema-2 session recovery remains unchanged.

**Qt adapter:** real `WebEngineView` and `QQuickWebEngineProfile` fixtures on the qualified Qt patch. Validate lifecycle, profile independence, downloads, frames, local archive guard, callbacks and teardown. Static QML source tests alone are insufficient.

**Wayland/native:** source-to-retention-to-destination reparent/focus on the primary platform, multiwindow shutdown, compositor activation, fractional scale, renderer failure and low-memory observation. Use existing xtask harness; add real test cases, not a new parallel build system.

### 23.2 Deterministic local fixture suite

Create a local synthetic server using the repository's existing fixture approach after inspecting xtask. It must use disposable profiles. Cases include:

- ordinary long document, two nested scrollers, expandable controls;
- classic form with text, hidden tokens, password, file and OTP fields;
- controlled-input SPA that overwrites DOM-only changes;
- contenteditable editor with unsaved text and blur-triggered autosave;
- lifecycle handlers that mutate on freeze/resume;
- continuously mutating document and navigation during capture;
- same-origin and cross-origin iframes, reordered/deleted frames, open/closed shadow roots;
- WebSocket, fetch polling and worker/service-worker activity;
- synthetic login with several related cookies, CSRF coupling and revocation on logout;
- cookie host-only/domain/path variations, identical names on different paths, expiry/session, Secure/HttpOnly/SameSite, partitioned variants where supported;
- malicious archive attempting remote CSS/image/frame loads, redirects, forms, popup/download/external-protocol navigation;
- renderer crash and profile teardown with a stash/capture callback pending.

Use HTTPS fixture origins for Secure/SameSite/partitioning tests with a test-only trusted certificate in disposable profiles. Do not disable certificate validation or sandboxing globally. No production domains, real credentials or external accounts in CI.

Google/Microsoft/GitHub sign-in smoke tests are manual, opt-in qualification with authorised test accounts. They are not required to run every unit suite, and success on one account flow is not a proof of universal authentication inheritance.

### 23.3 Existing quality gates

Run focused tests before the full gate. The inspected repository documents:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo xtask check --locked --offline
cargo xtask test adapter
cargo xtask test wayland
```

Use the last two for relevant native changes and report unavailable environment honestly. [R2] Discover current xtask fixture/test conventions before adding cases. Do not invent an already-existing test subcommand or claim to have executed one in this proposal.

### 23.4 Evidence records

Each slice has `docs/testing/versioned-browsing/<ID>.md` containing baseline/implementation SHA, environment, test commands and outputs, expected observations, failures, remaining gates and privacy impact. Update `docs/requirements.csv` using its real header and current accepted status vocabulary; do not invent a second requirements authority.

A capability certificate records Qt patch, Chromium version/security patch, platform/compositor, test fixture version, backend, exact scope and unsupported classes. Engine upgrades invalidate incompatible certificates until requalification.

---

## 24. Implementation phases and release milestones

The complete scope is broken into small slices in Appendix B and the JSON ledger. A phase is not one coding task.

| Phase | Goal | User-visible milestone |
|---|---|---|
| P0 | Baseline, ADR decisions, capability probes, fixtures | Honest status and reproducible evidence |
| P1 | Pure domain model, commands, scopes, IDs, diff/merge algorithms | Testable semantics without Qt side effects |
| P2 | Durable structural commits, journal, restore, listing | Useful preserve/restore workflow |
| P3 | Live leases, stash, resume, loss handling | Volatile live continuity |
| P4 | Archive capture, isolated viewing, bounded sidecars | Historical pages and conservative reconstruction |
| P5 | Clean branch graph, profile hosts, switch/disposal/export | Independent isolated exploration |
| P6 | Cookie backend observations, fidelity gates, conditional seeding | Verified inheritance only where support is real |
| P7 | Preview, conflict resolution, conditional controlled write-back | Advanced reconciliation where backend permits |
| P8 | Integration, accessibility, resource/fault qualification, release review | Supported feature set ready for rollout |

Dependency rules: P2 must not wait for cookie work. P3 must not wait for archives. Clean branching can begin from structural state after P3; page-enriched branching waits for P4. P6 observation and P7 pure planning can be implemented even when production seed/apply remains blocked. Native gates and dependency approvals are explicit constraints, not optional comments.

### 24.1 Completion levels

**Core complete:** structural commits/restore/diff, qualified live stashes, qualified archives/sidecars, clean branches/switch/disposal, explicit fidelity, privacy and regression gates all pass for the declared platform.

**Identity extension complete:** a backend passes exact declared snapshot/seed scope and the production feature is enabled only for that scope. An unsupported backend returns a useful reason.

**Reconciliation extension complete:** the planner, consent/preview flow and supported apply backend pass stale/partial/recovery tests. If the backend cannot safely meet the apply contract, this extension is explicitly blocked; do not advertise successful cookie merging.

**Proposal execution complete with blockers** is not the same as **full product implemented**. A goal-mode agent may finish all independent work and leave evidence-backed blocked slices. It must state which vision capabilities remain unavailable rather than silently redefine completion.

---

## 25. Agent execution protocol

1. Read repository instructions, this proposal's baseline map, the active slice and its dependencies. Check the current main/working branch; if ownership APIs moved, update the alignment evidence before coding.
2. Choose the smallest eligible not-started slice. Do not begin several unrelated features at once. A slice may be split into suffixed child IDs if it becomes too large, retaining traceability and dependencies.
3. Write or extend the focused failing test first where practical. Confirm the failure exercises missing behaviour, not a broken environment.
4. Implement only the slice. All new code follows existing state, command, storage and privacy boundaries. No opportunistic whole-repo refactor or root QML accumulation.
5. Run focused tests, formatting/lints and appropriate native checks. Record actual commands and unavailable checks, never inferred success.
6. Update the slice ledger/evidence and requirements traceability. Mark verified only against its acceptance conditions. A positive mock result cannot satisfy a native capability gate.
7. Review the diff for secret material, unintended profile writes, stale-target paths, unbounded queues and unsupported API calls.
8. Continue to the next dependency-ready slice only after the previous one is complete or has a concrete blocker. Do not install packages, use real accounts, publish branches or weaken safeguards without the environment's authority.

When a gate fails, record the exact reason, keep capability unavailable, add a regression that prevents false enabling, and proceed with unrelated slices. Do not substitute a private Qt API, arbitrary timeout or direct database copy to satisfy the appearance of completion.

A handoff should state: completed slice IDs, changed paths, tests run, native qualification outstanding, blockers, and the next eligible ID. Avoid unbounded narrative journals that a smaller agent must reread in full.

---

## 26. Maintainer acceptance checklist

The feature is accepted only when a maintainer can answer:

- What is durable, what is merely retained, and what is lost after a crash?
- Which owner/profile/document does every callback target?
- Does a new branch truly use a different engine profile?
- Can the original continue safely when child creation fails?
- Can a stash outlive tab-list/window changes without being silently destroyed?
- Does capture keep recovery and private browsing policies unchanged?
- Can an archive initiate unsolicited network/native actions?
- Are unsupported cookie identities and incomplete snapshots rejected?
- Does merge distinguish a pure plan from non-atomic engine application?
- Are failures and costs visible without leaking credentials?
- Can the main product ship while conditional identity features remain blocked?
- Is each advertised behaviour backed by a real test on the declared runtime?

The intended product remains ambitious. Its implementation should be conservative about guarantees, not conservative about usefulness.


---

## Appendix A. Sources and baseline audit
The repository was inspected through the connected GitHub tool. Source links below are pinned to the exact main commit, rather than moving `main` URLs. Inspection depth is recorded so partial source review is not presented as a full code audit. No Ferric build, native Qt test, browser automation or live-account test was executed while authoring this proposal.

Main was rechecked before packaging and remained [`deea458d92bcf6d8fbea7b04af17823b5882cc1b`](https://github.com/thomasaiwilcox/ferric-browser/commit/deea458d92bcf6d8fbea7b04af17823b5882cc1b).

Qt documentation was consulted on 27 September 2026; the served pages identify the Qt 6.11.2 family. That is documentation evidence, not proof of the user's installed runtime. The repository system-Qt ADR explicitly retains native qualification gates.

### R1 — Workspace and ownership boundaries
- [`Cargo.toml`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/Cargo.toml) — inspected: full.
- [`crates/ferric-browser-application/src/lib.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-application/src/lib.rs) — inspected: lines 1–230.
- [`crates/ferric-browser-runtime/src/lib.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-runtime/src/lib.rs) — inspected: lines 1–235.

### R2 — Contribution rules and verification commands
- [`CONTRIBUTING.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/CONTRIBUTING.md) — inspected: full.

### R3 — Development specification
- [`docs/DEVELOPMENT_SPEC.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/DEVELOPMENT_SPEC.md) — inspected: opening architecture sections and targeted searches; not entire file.

### R4 — Current core model and exports
- [`crates/ferric-browser-core/src/model.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-core/src/model.rs) — inspected: full.
- [`crates/ferric-browser-core/src/lib.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-core/src/lib.rs) — inspected: full.

### R5 — Recovery and private durability boundary
- [`docs/architecture/ADR-0006-profile-and-session-durability.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/architecture/ADR-0006-profile-and-session-durability.md) — inspected: full.

### R6 — Contexts, profiles and action subjects
- [`docs/architecture/ADR-0011-context-profile-window-actions.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/architecture/ADR-0011-context-profile-window-actions.md) — inspected: full.

### R7 — Journey retention and ephemeral teardown
- [`docs/architecture/ADR-0012-journey-and-ephemeral-state.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/architecture/ADR-0012-journey-and-ephemeral-state.md) — inspected: full.
- [`crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/qml/components/FerricWindowRegistryController.qml) — inspected: full.

### R8 — Application/storage coordination
- [`crates/ferric-browser-application/src/storage.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-application/src/storage.rs) — inspected: full.
- [`crates/ferric-browser-storage/src/worker.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-storage/src/worker.rs) — inspected: lines 1–200.

### R9 — Existing suspension eligibility
- [`docs/testing/m0-96-tab-suspension.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/testing/m0-96-tab-suspension.md) — inspected: full; its native status is not assumed verified.

### R10 — Current session schema and restore adapter
- [`crates/ferric-browser-storage/src/sessions.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-storage/src/sessions.rs) — inspected: lines 1–125.
- [`crates/ferric-browser-engine-qt/src/browser_ui_sessions.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/src/browser_ui_sessions.rs) — inspected: lines 1–200.

### R11 — Document-local scroll ownership
- [`docs/architecture/ADR-0014-explicit-scroll-target.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/architecture/ADR-0014-explicit-scroll-target.md) — inspected: full.

### R12 — Command and action metadata
- [`crates/ferric-browser-core/src/command.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-core/src/command.rs) — inspected: lines 1–190 and 230–385.
- [`crates/ferric-browser-core/src/action.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-core/src/action.rs) — inspected: lines 1–190.

### R13 — View ownership and same-profile transfer
- [`crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeSurface.qml) — inspected: targeted current-main code search; verify full surrounding path before editing.
- [`docs/testing/m0-157-tab-transfer-contract.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/testing/m0-157-tab-transfer-contract.md) — inspected: targeted current-main search.
- [`docs/testing/m0-160-secondary-tab-transfer.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/testing/m0-160-secondary-tab-transfer.md) — inspected: targeted current-main search.
- [`crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/src/browser_ui_tab_state.rs) — inspected: targeted current-main search.

### R14 — Qt composition, resources and native interceptor
- [`crates/ferric-browser-engine-qt/qml/Main.qml`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/qml/Main.qml) — inspected: full.
- [`crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeBase.qml`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeBase.qml) — inspected: lines 1–180.
- [`crates/ferric-browser-engine-qt/build.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/build.rs) — inspected: lines 1–150.
- [`crates/ferric-browser-engine-qt/src/request_interceptor.h`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-engine-qt/src/request_interceptor.h) — inspected: full public class declaration.

### R15 — System Qt distribution baseline
- [`docs/architecture/ADR-0003-system-qt-distribution.md`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/docs/architecture/ADR-0003-system-qt-distribution.md) — inspected: full.

### R16 — Configuration source layout
- [`crates/ferric-browser-config/src/browser_config.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-config/src/browser_config.rs) — inspected: directory metadata; implementation not read.
- [`crates/ferric-browser-config/src/settings.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-config/src/settings.rs) — inspected: directory metadata; implementation not read.
- [`crates/ferric-browser-config/src/validation.rs`](https://github.com/thomasaiwilcox/ferric-browser/blob/deea458d92bcf6d8fbea7b04af17823b5882cc1b/crates/ferric-browser-config/src/validation.rs) — inspected: directory metadata; implementation not read.

### Q1 — Qt Quick WebEngineView

[Official source](https://doc.qt.io/qt-6/qml-qtwebengine-webengineview.html). Lifecycle, save, script evaluation, mainFrame, renderer signals. Match signatures to the installed header/QML surface.

### Q2 — Qt WebEngine features

[Official source](https://doc.qt.io/qt-6/qtwebengine-features.html). Lifecycle freeze/resume and documented DevTools/remote-debugging behaviour.

### Q3 — QWebEngineScript

[Official source](https://doc.qt.io/qt-6/qwebenginescript.html). Injection points, ApplicationWorld and subframe policy.

### Q4 — webEngineFrame QML value type

[Official source](https://doc.qt.io/qt-6/qml-webengineframe.html). Frame lifetime, public evaluation, Qt 6.8 type and 6.10 child enumeration.

### Q5 — QWebEngineDownloadRequest

[Official source](https://doc.qt.io/qt-6/qwebenginedownloadrequest.html). Page-save formats, auto-start caveat, state and interruption handling. Quick integration must use the actual public QML surface.

### Q6 — QWebEngineCookieStore

[Official source](https://doc.qt.io/qt-6/qwebenginecookiestore.html). Public asynchronous cookie operations and observations; host-only origin rule. No enumeration-complete/batch transaction signature is documented.

### Q7 — QNetworkCookie

[Official source](https://doc.qt.io/qt-6/qnetworkcookie.html). Exposed identity and attributes; no exposed Chromium partition key in the documented type.

### Q8 — Qt WebEngine overview

[Official source](https://doc.qt.io/qt-6/qtwebengine-overview.html). Quick/Widgets architecture, process ownership and installed Chromium version reporting.

### Q9 — QQuickWebEngineProfile

[Official source](https://doc.qt.io/qt-6/qquickwebengineprofile.html). Public Quick profile cookie store and profile ownership/configuration surface.

### Q10 — Chrome DevTools Protocol Storage domain

[Official source](https://chromedevtools.github.io/devtools-protocol/tot/Storage/). Research-only alternate interface. Tip-of-tree protocol is not a stable production contract for the installed Qt Chromium build.

### Q11 — Chrome DevTools Protocol Network domain

[Official source](https://chromedevtools.github.io/devtools-protocol/tot/Network/). Cookie/partition-related protocol types. Richer metadata does not imply snapshot or batch atomicity.

### Q12 — WebEngineProfilePrototype

[Official source](https://doc.qt.io/qt-6/qml-qtwebengine-webengineprofileprototype.html). Quick profile construction since Qt 6.9; empty storage name, memory-only behaviour.

### Source interpretation limits

The source findings establish public API availability and architectural constraints. The detailed types, state machines, limits, SQL table design, command grammar and implementation sequencing are proposed Ferric design decisions, not claims copied from Qt. In particular, no completed external research report or unperformed runtime experiment is used as evidence. Native behaviour must be verified by the implementation slices.


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
