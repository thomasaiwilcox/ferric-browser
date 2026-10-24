# Dependency-ordered work inventory

60 execution-plan entries (59 work slices plus one coordinator aggregate) and 165 acceptance cases. All are unstarted.

| ID | Outcome | Depends on |
|---|---|---|
| S00 | Clone, reconcile latest source and freeze the execution map | — |
| S01 | Register production-backed feature test harness and baseline inventory | S00 |
| F00 | Add typed feature settings and validate defaults | S00 |
| F01 | Wire setting registry, atomic application and effective status | F00 |
| F02 | Extract/adapt the existing page factory without behaviour changes | S01 |
| F03 | Introduce explicit view role and generation guards | F02 |
| F04 | Qualify shared service and first-host lifetime | F02, S01 |
| F05 | Verify current per-window interaction parity and fix only gaps | F02, S01 |
| H00 | Implement single-spare policy and deterministic state tests | F00, S00 |
| H01 | Prepare an inert internal-mode view with bounded readiness | F03, H00, F01 |
| H02 | Adopt a matching spare through normal open routing | H01 |
| H03 | Eviction, resource pressure and teardown | H02, F04 |
| H04 | Add trustworthy hot/cold benchmark instrumentation | H02, S01 |
| H05 | Qualify unmapped window-mode spare independently | H03, W02 |
| H06 | Run native performance and constrained-machine acceptance | H03, H04, H05 |
| P00 | Prove isolated-world credential boundary on qualified Qt | S01, S00 |
| P01 | Create separately pinned helper build and dependency inventory | S00 |
| P02 | Implement pure credential admission and ceremony state | F00, S00 |
| P03 | Implement production document-bound native credential bridge | P00, P02, F03 |
| P04 | Implement mediator and public-key request serialization | P03 |
| P05 | Implement asynchronous helper supervision and bounded pipes | P02, P01 |
| P06 | Implement helper RP validation, PSL and request preparation | P01, P02 |
| P07 | Implement QR hybrid registration and assertion transport | P06 |
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
