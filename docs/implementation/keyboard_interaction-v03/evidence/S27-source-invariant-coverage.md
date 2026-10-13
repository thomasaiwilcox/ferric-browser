# S27 source and invariant coverage review

Reviewed against integrated source commit `7dbfb14af070c2494077ca6342ad009cefb41840`. `TRACEABILITY.json` contains 55 source-item mappings: A-01–A-20, F-01–F-08, B-01/B-10–B-13, and WEBAPP-001–022. The pack validator confirms those mappings, 24 contracts, 26 unified invariants, 57 planned test IDs, and 28 slices. The full mapping table below preserves each source item's assigned contracts and owner slices with their current ledger status.

“Implemented, unqualified” means the implementation and recorded local evidence exist while physical/live qualification remains open. A research no-go or environment block is not counted as a shipped capability.

## Unified normative invariants

| ID | S27 review and evidence |
| --- | --- |
| UI-001 | Preserved: existing defaults remain unchanged; S25–S27 made no production default or keybinding changes. See `S25.json`, `S26.json`, and the assessed source commit. |
| UI-002 | Bounded: the fresh T009 literal-label and control-value privacy fixtures pass; source review found no page-observation route that grants browser policy. These fixtures do not prove every page behavior. |
| UI-003 | Fixture/offscreen pass: native tests cover repeated and overlapping keys, receiver ownership, lost releases, and one-shot forwarding. `S27-native-qt-offscreen.log`; physical desktop input remains unqualified. |
| UI-004 | Local gate pass: workspace/engine tests passed for existing owner and revision paths. No live multi-window/tab journey was run. |
| UI-005 | Preserved: eligibility stays action-specific; proxy focus and label/control activation remain separate native tests. `S27-native-qt-offscreen.log`. |
| UI-006 | Fixture/offscreen pass: existing record, proxy, stale-scope and replacement tests passed; selection does not fall back to a same-name replacement. See `S27-engine-native.log`. |
| UI-007 | Bounded: the offscreen suite covers one-transport native activation and the S20 in-view drag probe/cleanup. S20 remains `blocked_environment`; no physical Wayland drag capability is claimed. |
| UI-008 | Local gate pass: owner-bound, one-shot local palette permit tests are included in the passing workspace/engine suites. No IPC/CLI authority was added by S27. |
| UI-009 | Local gate pass: typed configuration, validation, revision, and persistence suites passed in `cargo test --workspace --locked` and `cargo xtask check`. |
| UI-010 | Fixture/offscreen pass: Mixed policy and reserved-control routing tests passed; native test names include `mixedPageOwnerSurvivesPolicyAndEpochChanges` and `mixedBrowserOwnerSurvivesPageMappingChange`. |
| UI-011 | Fixture/offscreen pass: one-shot key forwarding and browser-owned chains/repeats passed; no buffered key replay path was introduced. `S27-native-qt-offscreen.log`. |
| UI-012 | Fixture/offscreen pass for tested document replacement/navigation invalidation. No broad claim is made for every same-document site transition. |
| UI-013 | Fixture/offscreen pass: prompt routing, recovery, deactivation, owner and epoch invalidation cases passed in the native suite. |
| UI-014 | Fixture pass: the T037 nested scroll target moves without Hint/search ownership changes; exact weak setter and empty/replaced scope no-fallback tests passed. `S27-engine-native.log`. |
| UI-015 | Fixture/offscreen pass: nested geometry and unsupported-transform rejection passed. Native Wayland is not run, so installed coordinate mapping remains unqualified. |
| UI-016 | Preserved: existing Hint target registry, text ranking, scroll setter, and pointer adapter remain the authorities; no replacement registry or wheel transport was introduced. T037 and S26 evidence apply. |
| UI-017 | Fixture pass: 1,000 start/cancel cycles with forced GC reported zero retained elements, observers, and windows. This is bounded WebEngine fixture evidence, not a live memory profile. |
| UI-018 | Fixture pass: visible target search, exact record meaning, confirmation, and replacement boundaries passed their existing tests. Virtualised live sites remain unqualified. |
| UI-019 | Capability-gated: S23 records `research_no_go` for the independent frame provider; current cross-origin and closed-root boundaries passed in the fresh WebEngine suite. |
| UI-020 | Fixture/research bounded: T009 privacy tests passed; S22–S24 provider investigations remain no-go. No page values or labels were added to persistent telemetry by S25–S27. |
| UI-021 | Blocked: S20 remains `blocked_environment`; S21 is `blocked_specification` because its conditional status rules conflict. No production drag UI exists. Cancellation cleanup does not imply rollback of site effects. |
| UI-022 | Capability-gated: S22 records `research_no_go`; editor focus is not promoted to generic rich writeback. |
| UI-023 | Measured, unresolved: S25 sampling met its warm-up/sample protocol, but callback p95 crossed the C17 relative investigation threshold and overlay p95 exceeded its reference ambition. See `S25.json` and raw S25 performance logs. |
| UI-024 | Preserved: missing physical/live tests remain `not_run` or blocked; neither startup smoke nor offscreen Qt is treated as Wayland. S27 reports the gaps explicitly. |
| UI-025 | Preserved: S25's deterministic model did not justify a label change; stable defaults and text search remain. S27's lint cleanup produced identical probe JSON. |
| UI-026 | Preserved: unresolved capability or environment gaps remain scoped to dependent slices; no substitute semantics or global support claim was added. See S20–S26 reports and `S27-readiness-summary.md`. |

## Source-item mapping recheck

The table is generated from the canonical traceability ledger and final `progress.json`; statuses are scheduling/evidence outcomes, not proof of site compatibility.

| Source item | Contract owners | Slice owners and current status |
| --- | --- | --- |
| A-01 | C01, C02 | S00 `complete` |
| A-02 | C02, C22 | S01 `implemented_unqualified` |
| A-03 | C02, C03, C04, C05 | S00 `complete`; S02 `implemented_unqualified`; S03 `implemented_unqualified` |
| A-04 | C03, C04, C05, C18 | S02 `implemented_unqualified`; S03 `implemented_unqualified`; S08 `implemented_unqualified` |
| A-05 | C04, C05, C08, C23 | S03 `implemented_unqualified`; S06 `implemented_unqualified` |
| A-06 | C03, C10, C11 | S07 `implemented_unqualified`; S08 `implemented_unqualified`; S09 `implemented_unqualified` |
| A-07 | C07, C11, C12 | S05 `implemented_unqualified`; S09 `implemented_unqualified`; S10 `implemented_unqualified` |
| A-08 | C04, C13, C14 | S13 `implemented_unqualified`; S14 `implemented_unqualified` |
| A-09 | C05, C14, C15 | S14 `implemented_unqualified`; S15 `implemented_unqualified` |
| A-10 | C16, C21 | S16 `implemented_unqualified`; S25 `research_no_go` |
| A-11 | C06, C19 | S04 `implemented_unqualified`; S19 `implemented_unqualified`; S20 `blocked_environment`; S21 `blocked_specification`; S26 `complete` |
| A-12 | C07, C20 | S05 `implemented_unqualified`; S22 `research_no_go` |
| A-13 | C13, C15, C20 | S13 `implemented_unqualified`; S15 `implemented_unqualified`; S23 `research_no_go`; S24 `research_no_go` |
| A-14 | C12, C18 | S10 `implemented_unqualified`; S18 `implemented_unqualified` |
| A-15 | C22 | S18 `implemented_unqualified`; S27 `implemented_unqualified` |
| A-16 | C17, C22 | S01 `implemented_unqualified`; S17 `implemented_unqualified`; S27 `implemented_unqualified` |
| A-17 | C22, C23, C24 | S00 `complete`; S27 `implemented_unqualified` |
| A-18 | C03, C04, C05, C10, C15, C18, C24 | S02 `implemented_unqualified`; S03 `implemented_unqualified`; S08 `implemented_unqualified`; S15 `implemented_unqualified`; S27 `implemented_unqualified` |
| A-19 | C20, C21, C24 | S22 `research_no_go`; S23 `research_no_go`; S24 `research_no_go`; S25 `research_no_go`; S27 `implemented_unqualified` |
| A-20 | C01, C24 | S18 `implemented_unqualified`; S27 `implemented_unqualified` |
| F-01 | C04, C08 | S01 `implemented_unqualified`; S02 `implemented_unqualified` |
| F-02 | C05, C06 | S03 `implemented_unqualified`; S04 `implemented_unqualified` |
| F-03 | C07, C20 | S05 `implemented_unqualified`; S22 `research_no_go` |
| F-04 | C22 | S01 `implemented_unqualified`; S27 `implemented_unqualified` |
| F-05 | C09 | S11 `implemented_unqualified`; S12 `implemented_unqualified` |
| F-06 | C17 | S01 `implemented_unqualified`; S17 `implemented_unqualified` |
| F-07 | C04, C08 | S02 `implemented_unqualified`; S06 `implemented_unqualified`; S17 `implemented_unqualified` |
| F-08 | C16, C21 | S16 `implemented_unqualified`; S25 `research_no_go` |
| WEBAPP-001 | C03 | S08 `implemented_unqualified` |
| WEBAPP-002 | C03 | S08 `implemented_unqualified` |
| WEBAPP-003 | C03, C10 | S08 `implemented_unqualified` |
| WEBAPP-004 | C10 | S08 `implemented_unqualified` |
| WEBAPP-005 | C05, C10 | S03 `implemented_unqualified`; S08 `implemented_unqualified` |
| WEBAPP-006 | C09, C10 | S08 `implemented_unqualified`; S12 `implemented_unqualified` |
| WEBAPP-007 | C03, C10 | S08 `implemented_unqualified` |
| WEBAPP-008 | C11 | S09 `implemented_unqualified` |
| WEBAPP-009 | C03, C07, C10 | S05 `implemented_unqualified`; S08 `implemented_unqualified` |
| WEBAPP-010 | C03, C14 | S14 `implemented_unqualified` |
| WEBAPP-011 | C04, C13 | S13 `implemented_unqualified` |
| WEBAPP-012 | C13, C20 | S13 `implemented_unqualified`; S23 `research_no_go`; S24 `research_no_go` |
| WEBAPP-013 | C15 | S15 `implemented_unqualified` |
| WEBAPP-014 | C04, C06, C15 | S02 `implemented_unqualified`; S04 `implemented_unqualified`; S15 `implemented_unqualified` |
| WEBAPP-015 | C16 | S16 `implemented_unqualified` |
| WEBAPP-016 | C12 | S10 `implemented_unqualified` |
| WEBAPP-017 | C19 | S19 `implemented_unqualified`; S20 `blocked_environment`; S21 `blocked_specification` |
| WEBAPP-018 | C20 | S22 `research_no_go` |
| WEBAPP-019 | C18 | S02 `implemented_unqualified`; S18 `implemented_unqualified` |
| WEBAPP-020 | C18, C22 | S18 `implemented_unqualified`; S27 `implemented_unqualified` |
| WEBAPP-021 | C17 | S17 `implemented_unqualified` |
| WEBAPP-022 | C22 | S27 `implemented_unqualified` |
| B-01 | C01, C02 | S00 `complete` |
| B-10 | C03, C04, C05, C24 | S02 `implemented_unqualified`; S03 `implemented_unqualified`; S27 `implemented_unqualified` |
| B-11 | C23 | S00 `complete`; S27 `implemented_unqualified` |
| B-12 | C22, C24 | S00 `complete`; S27 `implemented_unqualified` |
| B-13 | C02 | S00 `complete` |
