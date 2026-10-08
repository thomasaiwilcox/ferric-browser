# Versioned Browsing evidence and resumption map

This directory holds bounded implementation evidence, not a second requirements authority. The normative feature IDs are `VERSION-001`–`VERSION-011` in `docs/DEVELOPMENT_SPEC.md` and `docs/requirements.csv`. The 96 dependency-ordered work slices live in `CurrentImplementation/ferric-versioned-browsing/IMPLEMENTATION_BACKLOG.json`; `AGENT_HANDOFF.md` defines the execution loop. `capabilities.json` is the separate primitive-versus-production gate record. All production capability flags are currently false. A passing probe never enables a product command by itself.

The prior implementation campaign is recorded [here](final-campaign-2026-09-29.md). The 2026-10-05 workspace campaign is in [this report](final-campaign-2026-10-05.md). The current-tree verification continuation is in [this report](final-campaign-2026-10-06.md); it supersedes earlier “not run” notes only for checks included there. Pack validation and final goal reconciliation remain outstanding. Capability-specific acceptance gates and documented negative results remain open; a passing shared gate does not enable production features or approve release.

The intended independently supportable structural release boundary is drafted
in [structural-release.md](structural-release.md); VB-029 records its evidence
map and current campaign. Neither is a release approval until its own
acceptance criteria and dependencies are satisfied.

The upgrade and runtime-change policy is drafted in [engine-compatibility.md](engine-compatibility.md); its fingerprint and format regression fixtures passed in the final campaign. A second transition runtime is unavailable for cross-version execution, and native/UI integration remains open in [VB-093](VB-093.md).

Any per-slice note below that says checks were unrun records the implementation-phase deferral. The latest campaign report supersedes it for shared workspace, QML, storage/application, and bounded-probe results; it does not clear that slice's remaining native acceptance criteria.

| Requirement | Class / governing invariants | Implementation and test owner | Release evidence owner |
| --- | --- | --- | --- |
| VERSION-001 | Core structural; VB-I04, I09, I14, I18 | [VB-014](VB-014.md), current schema-3 manifest DTO with schema-1/2 read migrations [VB-018](VB-018.md), schema-4 index [VB-019](VB-019.md), durable object validation [VB-020](VB-020.md), structural-only transaction in progress [VB-021](VB-021.md), deletion preview and bounded GC in progress [VB-026](VB-026.md), verified allowlisted config policy preview/apply [VB-027](VB-027.md), normal-profile context scope [VB-028](VB-028.md), VB-022–029 | VB-029, VB-091, VB-094 |
| VERSION-002 | Core live; VB-I02, I03, I10, I11, I16, I18 | Pure retention leases [VB-030](VB-030.md), native view host [VB-031](VB-031.md), explicit-stash consent [VB-032](VB-032.md), staged-freeze work [VB-033](VB-033.md); primitive [VB-003](VB-003.md) | VB-033 is blocked: Qt's aggregate Frozen recommendation did not prove the open WebRTC data channel absent. Dependent slices VB-034–041 are explicitly blocked; production live continuity remains disabled. |
| VERSION-003 | Core page; VB-I04, I05, I10, I12, I14, I15 | Bounded sidecar DTO [VB-046](VB-046.md), top-document geometry [VB-047](VB-047.md), VB-042–045/VB-048–056; primitive [VB-004](VB-004.md), fixtures [VB-006](VB-006.md) | Current-tree native capture/save, sidecar, archive isolation and durable adapter probes passed. Capture/view gates remain false pending the separate VB-045/VB-057 acceptance matrix, unresolved viewless ordinary-download handling, and bounded native renderer evidence. |
| VERSION-004 | Core clean branch; VB-I03, I05, I08, I13, I16 | VB-058–068 | Blocked on the live-retention and hidden-owner prerequisites (VB-033–036); dependent branch slices are recorded blocked in the ledger. |
| VERSION-005 | Diagnostic observation; VB-I06, I07, I12, I17 | Backend contract [VB-070](VB-070.md), native Quick observation adapter [VB-071](VB-071.md), memory-only mirror [VB-072](VB-072.md), native descriptor qualification [VB-073](VB-073.md), negative source boundary [VB-074](VB-074.md), fixture-only seed contract [VB-075](VB-075.md), VB-077; negative public-API [VB-005](VB-005.md) | Observation is available for diagnostics. Branch delta/reporting and identity-decision slices depend on the blocked clean-branch inspection path. |
| VERSION-006 | Conditional identity; VB-I06, I07, I13, I17 | [VB-073](VB-073.md) confirms the current general identity/attribute gap and [VB-074](VB-074.md) rejects the public source boundary; [VB-075](VB-075.md) fixture-only seed/read-back contract, VB-076, VB-078; source gate currently unsupported | VB-076/078 are blocked because public Qt lacks general partition identity and a complete source snapshot boundary. Only a reviewed backend plus scoped native certificate can unblock them. |
| VERSION-007 | Pure planner; VB-I06, I08, I12, I18 | [VB-015](VB-015.md), preview permits [VB-016](VB-016.md), VB-080–082, VB-086 | Pure synthetic truth-table logic exists. Real branch-delta-bound plan, preview and stale-plan slices are blocked on branch-scoped source observations. |
| VERSION-008 | Conditional apply; VB-I06, I08, I12, I17, I18 | VB-083–088; current backend unsupported | Conditional apply and its dependent merge lifecycle are blocked; `versioning.cookie_merge_apply` stays unqualified and disabled. |
| VERSION-009 | Cross-cutting ownership/privacy; VB-I01–I05, I12, I15–I16 | VB-008–019, including operation protocol [VB-012](VB-012.md), typed runtime route [VB-017](VB-017.md), capture DTO [VB-018](VB-018.md) and normal-worker index [VB-019](VB-019.md); pure retention leases [VB-030](VB-030.md), native view host [VB-031](VB-031.md), explicit-stash consent [VB-032](VB-032.md) and staged-freeze host [VB-033](VB-033.md) (in progress); bounded untrusted sidecar [VB-046](VB-046.md), synthetic geometry collector [VB-047](VB-047.md), memory-only cookie contract [VB-070](VB-070.md), native observation queue [VB-071](VB-071.md), diagnostic mirror [VB-072](VB-072.md), loopback-only identity probe [VB-073](VB-073.md) and disposable persistent boundary probe [VB-074](VB-074.md), fixture-only seed/failure-destruction contract [VB-075](VB-075.md); VB-020–021, VB-042, VB-058, VB-090–091 | VB-091, VB-094 |
| VERSION-010 | Qualification/release; VB-I06–I07, I10, I17–I18 | VB-002–006, VB-090–093 | VB-094–095 |
| VERSION-011 | Structural versioning UI; VB-I04, I12, I14 | Verified keyboard UI [VB-025](VB-025.md) | Focused panel QML 18/18 and a bounded compositor-keyboard list → details → restore journey passed; independent screen-reader and broader VB-024 restore qualification remain open |

VB-042–056 were advanced as one continuous implementation pass. The Rust-held one-use consent dialog, bounded storage handoff, exact page-save route, archive/object publication, frame-aware sidecar, and geometry/editable/media recovery paths exist in code with authored regressions. Exact archive materialization is connected to a dynamic isolated WebEngine surface; it waits for blank acknowledgement before exact-file cleanup. The 2026-10-05 campaign passed the current workspace, QML, storage, application, native archive/capture and lint gates. Production capture/view flags remain false: viewless ordinary-download ownership and full release qualification remain open, and Qt/Chromium suppression of some archive resources is not per-resource interceptor evidence.

VB-024 now routes one-page and multi-page live restores into a fresh, tab-capable primary browser surface, preserving source isolation and loading only the selected safe URL. The selected page's optional geometry/editable/media sidecar path is connected through an exact-tab coordinator in that destination. Current-window replacement is still unavailable until the live-retention gate in VB-033 can prove preservation of unknown page state.

The first four classes are the independently useful release core. The diagnostic observation and pure planner can progress without a complete cookie backend. Snapshot identity and controlled write-back are conditional extensions: [VB-005](VB-005.md) records unsupported general identity/completeness on the current public Qt backend, so no general cookie snapshot or merge application is advertised. A future backend needs its own reviewed decision and native certificate; a quiet callback interval, mock, source-string test or successful build is insufficient.

## Continue implementation

1. Inspect `git status`, HEAD, applicable repository instructions and the selected slice's dependency evidence. Preserve unrelated work.
2. Continue every `in-progress` slice that can still advance and the lowest eligible `not-started` slice. Record concrete dependency/capability blockers in the ledger and continue independent work. Do not stop for per-slice checkpoints when the current request is continuous implementation.
3. Read that task, its referenced proposal sections and current source. Add a focused failing test where practical; implement only the selected responsibility.
4. Normally run focused checks and any relevant native gate. If the current user explicitly defers verification, author and maintain focused fixtures but hold their execution until the implementation pass reaches its final campaign. Record unavailable environments as skipped/unqualified, not as success. Do not use private Qt APIs, normal user profiles or live accounts to force a pass.
5. Write `VB-<ID>.md` using the template below, update `docs/requirements.csv` and the JSON ledger, then refresh the ledger entry in `SHA256SUMS.txt`. Run `sha256sum -c SHA256SUMS.txt` from the pack directory, `validate_pack.py`, `git diff --check` and the relevant repository checks. Do not mark the slice verified until its actual acceptance criteria pass.

## Evidence template

```markdown
# VB-NNN — Title

Status: not-started | in-progress | verified | blocked
Baseline SHA:
Implementation SHA or uncommitted-tree description:
Affected requirement and invariant IDs:

## Behavior or investigation

## Focused tests
- Command, environment, actual result (including failure or skip):

## Native evidence, when required
- Qt/QtWebEngine/Chromium security patch, platform/compositor, fixture and backend:
- Exact observation and limit of the claim:

## Privacy and ownership review

## Remaining blockers and unblock condition

## Next eligible slice
```

The template is a reporting aid, not proof. Keep cookie values, editable content, production URLs and profile files out of evidence. The [capability record](capabilities.json) and each slice's observed tests remain the authoritative gates for native claims.
