# Sources, coverage and reconciliation decisions

## What was merged

**Source A:** `sources/v02-original.md`, copied without editing from the mounted `ferric_webapp_keyboard_interaction_proposal_v02.md`. It is the full 6,092-word input, not the shortened inline rendering from the earlier answer. Its 20 sections and 22 WEBAPP invariants are traced in `TRACEABILITY.json`.

**Source B:** “Ferric Browser — Targeted Keyboard Interaction Recommendations”, pasted by the user in the request. Its F-01–F-08 identifiers cover original sections 2–9. B-01 and B-10–B-13 cover purpose, guardrails, sequencing, definition of done and source anchors. The user-pasted original remains the source; this pack supplies a requirement inventory, not a purported byte-identical copy of that message.

The two sources share Ferric baseline `4337056`. No older scroll-target proposal or the concurrent versioning proposal is silently added as another source of requirements.

## Coverage ledger

| Source requirement | Canonical ownership | Planned slices |
| --- | --- | --- |
| A: executive vision, failure taxonomy and baseline | C01–C03 | S00–S01 |
| A: existing modes, scroll targets and architectural authorities | C03–C05 | S02–S03; regression gates throughout |
| A: Mixed exact-key policy and recovery | C10 | S07–S08 |
| A: SPA/document lifetime | C11 | S09 |
| A: context/native prompt leases | C12 | S10 |
| A: regions and surfaces | C13 | S13 |
| A: scoped Hints/scroll actions | C14 | S14 |
| A: region-scoped Grid | C15 | S15 |
| A: visible Target Search | C16 | S16 |
| A: diagnostics and optional interaction profiles | C18 | S18 |
| A: double-click and bounded drag | C19 | S19–S21 |
| A: rich editor adapters | C20 | S22; narrow focus is S05 |
| A: independent child-frame provider | C20 | S23 |
| A: accessibility provider | C20 | S24 |
| A: wheel/local scrolling opportunity | C19 | S26, investigation only |
| A: live journeys, performance, test strategy and completion | C17, C22–C24 | S01, S17, S27 |
| F-01: investigate shadow-root final-activation mismatch | C04 | S01–S02 |
| F-02: controlled native-pointer activation | C05–C06 | S03–S04 |
| F-03: hidden editor proxies and Bootstrap controls | C07 | S05; advanced-framework investigation S22 |
| F-04: permanent compatibility regression matrix | C22 | S01 and every feature slice; integrated S27 |
| F-05: declarative scoped selectors | C09 | S11–S12 |
| F-06: measure and bound hot path | C17 | baseline S01; integrated optimisation S17 |
| F-07: shared primitives and readable script assets | C04, C08 | minimal S02; extraction S06; measured runtime optimisation S17 |
| F-08: optional label ergonomics versus existing text search | C16, C21 | reuse S16; evaluation S25 |
| B: guardrails, independent slices and definition of done | C23–C24 | all cards and S27 |

`TRACEABILITY.json` additionally maps each WEBAPP-001–022 source invariant to its replacement contract and owner slices. Original wording that implied impossible atomicity or pinned native DOM recipients is reconciled below; it is not silently copied into contradictory requirements.

## Explicit conflict and ambiguity resolutions

| ID | Overlap or conflict | Unified decision |
| --- | --- | --- |
| D01 | New discovery work versus repairs to existing final actions | Foundation tests/repairs precede new target workflows; Mixed's pure model can be prepared independently but not ship ahead of its native contract. |
| D02 | Proposal A's new search surface versus existing Hint text search noted by B | New direct entry and presentation, same HintInteraction/ranking/candidate authority. No new matcher or durable index. |
| D03 | One shared visibility function versus invisible proxy requirements | Shared low-level primitives, action-specific visible representative/focus/pointer/scroll predicates. No blanket visibility bypass. |
| D04 | “Native click cannot retarget” versus the JS/native interval | Preserve exact identity for semantic actions; reject observable native invalidation; explicitly disclose residual non-atomicity and keep native click opt-in. |
| D05 | Grid/native machinery reuse versus Grid's local-only ADR | Narrow local physical palette permit handoff with an ADR amendment; no IPC, macros, scripts or generic discovery coordinate injection. |
| D06 | Hidden ACE textarea and Bootstrap custom input grouped together | EditorFocus pairs and LabelControl pairs are different types/actions. A checkbox is not a rich-editor input proxy. |
| D07 | Proxy focus versus rich-editor external editing | Focus does not grant extract/apply. Framework writeback remains separately consented, versioned and capability-gated. |
| D08 | Configurable selectors versus final validation hard-coded to defaults | Capture effective policy/provenance; collection, fresh and final checks use the same policy revision. Selectors never grant actions or proxy exceptions. |
| D09 | Full CSS syntax and atomic config versus a pure Rust config crate | Prepare in Rust, validate in real engine parser, commit once using revision guard; no Qt dependency in config and no partial/stale activation. |
| D10 | Region root expressed as CSS/DIP rectangle | Distinct owner CSS, top CSS and Qt ViewLocal coordinate contracts; test conversion; reject unsupported transforms. |
| D11 | “Cancel on repaint” versus useful video/canvas Grid | Cancel on observed owner/surface/geometry/scroll invalidation, not arbitrary repaint. Canvas internals remain a visual/racy fallback. |
| D12 | Gestures owned end-to-end versus actual native focus moves | Keep routing ownership stable and no replay/manual redirection; do not promise the original DOM receiver gets a release after native focus changes. |
| D13 | Any-URL pass-through reset versus SPA editing comfort | Change only with positive same-document proof. Otherwise preserve current safe reset. One-shot stays stricter. |
| D14 | Prompt-restoration epoch compared before a prompt that increments epoch | Record the expected suspended epoch and validate owner on completion; never resurrect one-shot/Hint/Grid sessions. |
| D15 | Duplication of fixture/benchmark infrastructure | One matrix with stable test IDs; each slice contributes its own tests; final audit consolidates evidence, not postpones tests. |
| D16 | Broader JS refactor mixed with a correctness repair | Minimum shared extraction in S02; behavioural-parity assets in S06; caching only after measurements in S17. |
| D17 | New selector/key/proxy/label options versus stable defaults | Existing defaults retained; proxy exception and profiles are opt-in; Mixed is exact-key Normal-only; alternative labels are research-only. |
| D18 | “Every suggestion included” versus not every advanced feature has a safe API | All items have owners. Research no-go/unavailable results are separate from feature completion; conditional code cannot silently expand scope. |
| D19 | “No assumptions” versus unknown real-site/framework behaviour | Fixed contracts and budgets; tests, explicit fallback and blocking rules handle unknowns. No invented support or API to fill a gap. |
| D20 | Small-model implementation versus many interdependent schemas | One coordinator, serial-by-default DAG, bounded read/write scopes, shared locks, durable evidence/progress and one launch prompt. |

Chosen settings, bounds, action naming, permitted provenance and exact failure behaviour in v0.3 are **new reconciliation decisions**. They are not claims that either input already specified these details or that the current code already implements them.

## Ferric source anchors actually re-read during this merge

All repository links here pin the same immutable baseline. The cited output establishes source structure, not runtime test results.

* **R01 — baseline:** [commit 4337056](https://github.com/thomasaiwilcox/ferric-browser/commit/4337056f632de0c710943fa6df76c67cf14b8b47). The live `main` branch ref matched this commit during preparation.
* **R02 — candidate and final-action code:** [BrowserScripts.js, collection/fresh/focus/click](https://github.com/thomasaiwilcox/ferric-browser/blob/4337056f632de0c710943fa6df76c67cf14b8b47/crates/ferric-browser-engine-qt/qml/scripts/BrowserScripts.js#L282-L411). Blob SHA returned by the connector: `5d0271f8deead4803609bb4675310566d008c06a`. Final-action shadow-root inconsistency remains suspected until a fixture reproduces it.
* **R03 — existing text-mode authority:** [hints.rs, HintInteraction](https://github.com/thomasaiwilcox/ferric-browser/blob/4337056f632de0c710943fa6df76c67cf14b8b47/crates/ferric-browser-core/src/hints.rs#L366-L540). Blob SHA: `d4b4ce3c305a8bc3395f5f014c47aaa69e9c23a9`.
* **R04 — native spatial authority:** [ADR-0013](https://github.com/thomasaiwilcox/ferric-browser/blob/4337056f632de0c710943fa6df76c67cf14b8b47/docs/architecture/ADR-0013-spatial-grid-navigation.md). Blob SHA: `76863aafbcb7d0a56fdb437b70265016f13e7261`. The pointer adapter filenames were also confirmed through the connected repository search.
* **R05 — closed Hint grammar:** [hint_policy.rs](https://github.com/thomasaiwilcox/ferric-browser/blob/4337056f632de0c710943fa6df76c67cf14b8b47/crates/ferric-browser-engine-qt/src/hint_policy.rs#L1-L200). Blob SHA: `2a7e9b1e26e015a052ab5766f65ced7dd682588d`.

Other anchor paths in slice cards are the existing baseline paths reported by the two source proposals and earlier verified source context. S00 must verify their current local location before edits. New files are proposed locations, not asserted existing APIs.

## External technical checks (separate from the two proposals)

* **E01 — HTML activation:** [WHATWG HTML, activation behaviour](https://html.spec.whatwg.org/multipage/interaction.html#dom-click). The standard distinguishes scripted click activation from trusted user activation. This supports retaining engine restrictions; it does not qualify Ferric's native adapter.
* **E02 — Qt script/frame APIs:** [QWebEngineScript](https://doc.qt.io/qt-6/qwebenginescript.html), [QWebEngineFrame](https://doc.qt.io/qt-6/qwebengineframe.html), [QML webEngineFrame](https://doc.qt.io/qt-6/qml-webengineframe.html). Public script-world and frame APIs exist with version-specific boundaries; availability in documentation is not a Ferric compile/runtime qualification.
* **E03 — CSS selector parsing:** [WHATWG DOM, selector scope matching](https://dom.spec.whatwg.org/#scope-match-a-selectors-string). Engine selector parsing/matching—not regex approximation—is the syntax authority. Atomic application and resource budgets are this pack's design decisions.

**Source B's qutebrowser anchors:** [tree at 2bd7591](https://github.com/qutebrowser/qutebrowser/tree/2bd7591), `qutebrowser/browser/hints.py`, `qutebrowser/javascript/webelem.js`, `qutebrowser/browser/webelem.py`, selector settings and hint end-to-end tests. They remain comparison inputs from the supplied inspection, not a new reproduced qutebrowser defect list.

## What was and was not validated while preparing the pack

Read the mounted full v0.2 file, inspected current pinned Ferric source through the connected GitHub tool, cross-mapped both proposals, checked selected public platform documentation, and ran the pack's structural/negative validation tests and TOML example parsing.

Did **not** compile Ferric, execute the new planned tests, run native Wayland, test Google Docs, reproduce the suspected shadow bug, benchmark Ferric, or qualify qutebrowser/framework behaviour. Empty progress/evidence templates intentionally do not claim any such results.
