# Slice index

Read the individual card and named contract sections. The default coordinator works serially; lock groups also restrict optional parallel work. Research/no-go outcomes never imply shipped features.

| Slice | Kind | Depends on | Outcome |
| --- | --- | --- | --- |
| [S00](slices/S00.md) | foundation | — | Baseline, dependency and symbol audit |
| [S01](slices/S01.md) | foundation | S00 | Compatibility harness and suspect-defect reproduction |
| [S02](slices/S02.md) | implementation | S01 | Shared composed validation and final-action correctness |
| [S03](slices/S03.md) | implementation | S02 | Action intent, typed provenance and palette handoff |
| [S04](slices/S04.md) | implementation | S03 | Explicit native-pointer hint activation |
| [S05](slices/S05.md) | implementation | S02, S03 | Validated EditorFocus and LabelControl pairs |
| [S06](slices/S06.md) | implementation | S02, S04, S05 | Incremental versioned script asset extraction |
| [S07](slices/S07.md) | implementation | S00, S01 | Pure exact-key Mixed policy model |
| [S08](slices/S08.md) | implementation | S03, S07 | Native Mixed ownership before QML special keys |
| [S09](slices/S09.md) | implementation | S08 | Same-document pass-through lifecycle qualification |
| [S10](slices/S10.md) | implementation | S03, S08, S09 | Native prompt leases and bounded context |
| [S11](slices/S11.md) | implementation | S00, S06 | Selector configuration prepare/validate/commit boundary |
| [S12](slices/S12.md) | implementation | S06, S11 | Site-scoped selectors through discovery and activation |
| [S13](slices/S13.md) | implementation | S03, S05, S06, S12 | Regions and surfaces on the existing registry |
| [S14](slices/S14.md) | implementation | S13 | Shared scope leases and scoped Hints |
| [S15](slices/S15.md) | implementation | S04, S10, S14 | Region-scoped Grid model and native mapping |
| [S16](slices/S16.md) | implementation | S06, S12, S14 | Visible Target Search as a Hint text-mode view |
| [S17](slices/S17.md) | implementation | S06, S12, S15, S16 | Measured hot-path optimisation and teardown |
| [S18](slices/S18.md) | implementation | S08, S10, S12, S15, S16, S17 | Interaction diagnostics and optional data profiles |
| [S19](slices/S19.md) | gated_implementation | S04, S10, S15 | Native double-click capability and explicit action |
| [S20](slices/S20.md) | research | S04, S10, S15 | Bounded in-view drag feasibility gate |
| [S21](slices/S21.md) | gated_implementation | S20 | Conditional bounded drag implementation |
| [S22](slices/S22.md) | research | S05, S06, S18 | Framework editor capability and writeback design |
| [S23](slices/S23.md) | research | S14, S18 | Independent child-frame provider spike |
| [S24](slices/S24.md) | research | S14, S18 | Accessibility-provider and assistive-technology spike |
| [S25](slices/S25.md) | research | S16, S17 | Optional labels versus existing text search |
| [S26](slices/S26.md) | research | S17, S18 | Wheel-only task gap evaluation |
| [S27](slices/S27.md) | release | S18, S19, S21, S22, S23, S24, S25, S26 | Integrated release audit and task qualification |
