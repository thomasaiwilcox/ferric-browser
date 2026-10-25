# Dependency-ordered work inventory

83 execution-plan entries (74 work slices plus nine coordinator aggregates) and 172 acceptance cases. Mutable execution status is recorded in the external run ledger.

| ID | Outcome | Depends on |
|---|---|---|
| S00 | Clone, reconcile latest source and freeze the execution map | — |
| S01 | Register production-backed feature test harness and baseline inventory | S00 |
| F00 | Add typed feature settings and validate defaults | S00 |
| F01 | Wire setting registry, atomic application and effective status | F00 |
| F02 | Aggregate page-factory, callback and focus outcomes | F02a, F02b |
| F02a | Extract page factory and qualify document-owner callbacks | S01 |
| F02b | Aggregate secondary-host focus and popup/transfer outcomes | F02b1, F02b2 |
| F02b1 | Diagnose and qualify secondary-host keyboard routing | F02a |
| F02b2 | Aggregate popup command and transfer reserve outcomes | F02b2a, F02b2b |
| F02c | SDK-qualified native opener lifetime leases | F02b1 |
| F02d | Production popup adoption/retirement wiring | F02c |
| F02b2a | Unchanged T014a no-code qualification after dependency repair | F02b1, F02d |
| F02b2b | Diagnose and qualify popup transfer reserve hover preservation | F02b2a |
| F03 | Introduce explicit view role and generation guards | F02 |
| F04 | Qualify shared service and first-host lifetime | F02, S01 |
| F05 | Verify current per-window interaction parity and fix only gaps | F02, S01 |
| H00 | Aggregate bounded spare policy outcomes | H00a, H00b, H00c |
| H00a | Implement bounded spare permit and deadline transitions | F00, S00 |
| H00b | Qualify atomic exactly-once spare claim | H00a |
| H00c | Qualify complete environment identity and exclusion | H00b |
| H01 | Prepare an inert internal-mode view with bounded readiness | F03, H00, F01 |
| H02 | Adopt a matching spare through normal open routing | H01 |
| H03 | Eviction, resource pressure and teardown | H02, F04 |
| H04 | Add trustworthy hot/cold benchmark instrumentation | H02, S01 |
| H05 | Qualify unmapped window-mode spare independently | H03, W02 |
| H06 | Run native performance and constrained-machine acceptance | H03, H04, H05 |
| P00 | Aggregate isolated-world credential boundary proof | P00a, P00b, P00c |
| P00a | Prove isolated transport and main-frame authority facts | S01, S00 |
| P00b | Reject stale and cross-document credential completions | P00a |
| P00c | Resolve the feasible public Qt API route and document limits | P00b |
| P01 | Create separately pinned helper build and dependency inventory | S00 |
| P02 | Aggregate credential policy outcomes | P02a, P02b, P02c |
| P02a | Implement closed credential owner and pending RP candidate types | F00, S00 |
| P02b | Implement exactly-once credential ceremony terminal transitions | P02a |
| P02c | Qualify credential admission and provider isolation | P02b |
| P03 | Implement production document-bound native credential bridge | P00, P02, F03 |
| P04 | Implement mediator and public-key request serialization | P03 |
| P05a | Native helper supervisor component | P01, P02 |
| P05b1 | Non-admission protocol and private native Rust bridge | P05a, P02 |
| P05b2 | Original admitted application route/qualification | P05b1, P00, P03 |
| P05b | No-code protocol/admitted-app aggregate | P05b1, P05b2 |
| P05 | No-code helper supervision/application aggregate | P05a, P05b |
| P06 | Implement helper RP validation, PSL and request preparation | P01, P02 |
| P07a | Sealed real hybrid operation core and supported virtual tests | P06, P01 |
| P07b | Bounded helper pipe/progress/result integration | P07a, P06 |
| P07 | No-code real hybrid/helper aggregate | P07a, P07b |
| P08 | Enforce helper egress and Bluetooth capability policy | P06, P05 |
| P09 | Build native provider chooser and preserve native authentication | P04, P05, F05 |
| P10 | Implement owned QR/progress UI and accessible cancellation | P09, P07 |
| P11 | Complete WebAuthn response objects and capability semantics | P04, P07, P09 |
| P12 | Wire complete credential lifecycle, abort and document invalidation | P03, P05, P09, P10, P11, F04 |
| P13 | Create independent relying-party verification fixture | S01, P06 |
| P14 | Run composed phone flow and hostile bridge integration tests | P08, P11, P12, P13 |
| P15 | Fuzz credential boundaries and verify resource/privacy caps | P14 |
| P16 | Independent credential security and conformance review | P15, P14 |
| P17 | Physical iPhone, Android and native-key qualification | P16, P13 |
| W00 | Prove exact native-host correlation and grouping primitives | S00, S01 |
| W01 | Aggregate presentation/open-target policy children | W01a, W01b, W01c |
| W01a | Map explicit page-open targets to typed presentation intents | F01b, S00 |
| W01b | Preserve profile, privacy and captured targets through context confirmation | W01a |
| W01c | Keep destructive command effects within their explicit scopes | F01b, S00 |
| W02 | Create single-page native hosts using existing workspaces | W01, F02, F04, F05 |
| W03 | Implement tab traversal, close, pin and undo in window mode | W02 |
| W04 | Add bounded compositor observation and capability layer | W00 |
| W05 | Bind actual host IDs to compositor clients | W04, W02 |
| W06 | Implement transaction-scoped grouped foreground creation | W05, W03 |
| W07 | Preserve background creation, popup and standalone semantics | W06, W02 |
| W08 | Use observed group order for non-destructive traversal | W06, W03 |
| W09 | Extend session presentation metadata compatibly | W02, W03 |
| W10 | Implement bounded restore and presentation conversion | W09, W02 |
| W11 | Qualify context/private isolation and prompt ownership | W02, W07, F04 |
| W12 | Separate visibility, focus and renderer lifecycle | W04, W02 |
| W13 | Handle compositor disconnect and grouping recovery | W06, W08, W12 |
| W14 | Qualify actual Hyprland and generic Wayland experience | W07, W08, W10, W11, W12, W13 |
| X00 | Add cross-feature composition and lifecycle stress matrix | H05, P14, W10, W11, W13 |
| X01 | Package helper, PSL, system prerequisites and license inventory | P07, P08, F01 |
| X02 | Integrate new suites and helper checks into CI | X00, X01, P15 |
| X03 | Generate user references, diagnostics and recovery guidance | F01, W03, P10, H04 |
| X04 | Independent integrated architecture and regression review | X00, X02, X03, F04, F05 |
| X05 | Run safe build cleanup and reproducible rebuild checkpoint | X04 |
| X06 | Final capability qualification and handover | X05, H06, P17, W14 |
