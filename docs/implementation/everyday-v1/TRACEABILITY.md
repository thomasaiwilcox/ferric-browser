# Proposal-to-execution traceability

The full original proposal is in reference/ORIGINAL_PROPOSAL.md. Contracts and cases below carry its required behaviour forward while updating implementation anchors. Do not mistake the original proposal for a file-by-file instruction against the latest source.

| Original work/outcome | Current contract | Implementation and acceptance owners |
|---|---|---|
| Latest baseline, ownership and defaults | C00,C01,C13 | S00,F00,F01; initial state and source map |
| Shared page factory and view lifecycle | C01,C02 | F02 aggregate (F02a T015, F02b aggregate: F02b1 T013 and F02b2 aggregate (F02b2a T014a, F02b2b T014b)), F03,F04 |
| Secondary keyboard parity | C01,C09 | F05 verifies/reuses current HintWorkspace/PageSearch, not old disabled-hint premise |
| One speculative hot view, no external prefetch | C02,C03 | H00 aggregate (H00a/T025, H00b/T026, H00c/T027), H01–H03 |
| Hot native-window variant and measured adoption | C03 | H04–H06 |
| Origin/document-bound isolated phone bridge | C04 | P00 aggregate (P00a/T046, P00b/T047, P00c/T048), P02–P04; security review P16 |
| Helper transport reuse and supervised protocol | C06,C07 | P01,P05–P08 |
| Native provider choice, QR and response compatibility | C05,C08 | P09–P12 |
| Independent RP, adversarial/fuzz/device evidence | C14 | P13–P17 |
| Internal/window choice without two browser cores | C09 | W01–W03 |
| Actual compositor primitives and exact identity | C10 | W00,W04–W07 |
| Observed group order/focus/visibility | C11 | W08,W12,W13 |
| Session metadata, bounded restore and privacy | C12 | W09–W11 |
| Real Hyprland qualification | C10,C11,C14 | W14 |
| Cross-feature cancellation/lifetime | C01,C12,C14 | X00 |
| Packaging, CI, documentation and independent integration review | C13,C14 | X01–X04 |
| Resource cleanup and reproducible handover | C00,C14 | X05,X06 |

Machine-readable EF-* requirements map one outcome to an accountable slice; each work slice maps to its owned concrete T* acceptance cases; coordinator aggregates cite their children. Each case must be bound to actual implementation tests or specified independent/native/device evidence. Task-level cases are a minimum, not a replacement for the detailed edge cases within the contracts.

Source-driven amendments: current extracted QML controllers, shared profile lifetime registry, captured EngineRequests dispatch, Qt6.11.2/private download-source guard, Widgets/KF6WindowSystem6.28, helper Rust1.88 versus root1.85, and existing per-window hints. These are included in BASELINE and S00 reuse decisions.
