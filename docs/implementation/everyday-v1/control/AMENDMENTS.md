# Coordinator plan amendments

## A09 — F00 Config composition owner path

- **Status:** independently reviewed and accepted by `/root/a09_reviewer`; product implementation may proceed after the amended integration commit is recorded.
- **Trigger:** F00 source tracing showed that the top-level `Config` fields and `impl Default for Config` are defined in `crates/ferric-browser-config/src/runtime_config.rs`; F00's original five-path allowlist could not add the required defaulted `[performance]` and `[passkeys]` sections or wire the existing tabs/hyprland fields.
- **Change:** add `runtime_config.rs` to F00 `allowed_paths`, `read_anchors`, and the slice's exact path list. F00 now allows 6 product files, below the 8-file limit. Add F00 to the existing source-map path record.
- **Evidence:** current `Config` at `runtime_config.rs:20` and `impl Default for Config` at `:82`; this same file is already an F01 anchor.
- **Dependency/lock effect:** no graph or lock changes. F01 already depends on F00, so its later access to `runtime_config.rs` remains serial. F00 retains its `config-model` lock.
- **Test effect:** T007–T009 and the existing focused `cargo test -p ferric-browser-config --locked` gate remain unchanged. No behavior/default/security requirement is weakened.


## A10 — Correct S01 CMake entry point and skip-visible gate

- **Status:** independently reviewed and accepted by `/root/a09_reviewer` during S01 attempt-2 review.
- **Trigger:** S01 review found that `tests/everyday/CMakeLists.txt` is an include-only child and requires the production `application_tests` target; configuring that child directly cannot pass. Review also found that test output must expose QtTest skip totals for the required-test guard.
- **Change:** baseline recipe now configures `crates/ferric-browser-engine-qt/tests/application`, which creates `application_tests` and includes the everyday harness. It supplies the actual Cargo-built engine archive from the registered external target directory. The required S01 acceptance run uses verbose CTest under `run_gate.py --require-tests`; `T005` discovery output retains test counts and names.
- **Dependency/lock effect:** no graph, product-file scope, or dependency change. Coordinator used `integration` and `plan` locks to correct the execution recipe.
- **Test effect:** no acceptance behavior is waived. A zero-test result or visible skip fails the required gate. No feature, security, default, or test assertion is weakened.


## A11 — F00 diagnostics config literal

- **Status:** independently reviewed and accepted by `/root/a09_reviewer`; F00 attempt 2 may proceed.
- **Trigger:** F00 attempt-1 review found that adding `HyprlandConfig.page_grouping` makes the existing struct literal in `crates/ferric-browser-engine-qt/src/diagnostics_overview.rs` incomplete. The literal is outside F00's current six-file allowlist and would block compilation.
- **Change:** add `diagnostics_overview.rs` to F00 `allowed_paths`, `read_anchors`, the slice's exact edit list, and the execution source map. F00 now allows 7 product files, below the 8-file limit.
- **Dependency/lock effect:** no graph or lock changes. F00 retains `config-model`; later X03 access remains serial under its task dependency/locks.
- **Test effect:** T007–T009 and the focused config crate gate are unchanged. The added path updates the existing diagnostic config literal to compile with the new typed field; no requirement or security behavior is relaxed.


## A12 — Split F01 at existing UI adapter boundaries

- **Status:** independently reviewed and accepted by `/root/a09_reviewer` on review attempt 2; F01 core implementation may proceed under the amended split.
- **Trigger:** F01 source tracing found two existing UI publication call sites outside the original eight-path product allowlist: `browser_ui_defaults.rs` initializes `FeaturePreferences`, and `browser_ui_configuration.rs` publishes the projection. The current `FeaturePreferences`, `bridge.rs`, and `lib.rs` fields cannot reach QML without both call sites. F01 T012 also depends on hot-page and phone-request consumers supplied by later H01/P12 work.
- **Change:** split F01 into core config/atomic apply (four product paths plus generated `docs/user/configuration.md`), F01b typed UI projection (six product paths), and F01c disable-semantics verification (two registered test paths). Keep the eight-product-file cap for each implementation slice. Add both adapter paths to the execution source map. Move T011 to F01b and T012 to F01c without changing either expected behavior.
- **Dependency effect:** F01b depends on F01; F01c waits for F01b, H01, P12 and S01. H01, W01 and P03 depend on F01b so consumers cannot use the new settings before requested/effective state reaches the UI.
- **Test effect:** T010 remains in F01; T011 is tested in F01b; T012 remains intact and is exercised across the H01 hot-preparation and P12 credential lifecycle tests. No positive or disable assertion is waived.

## A13 — F02 production-backed page-factory test registration

- **Status:** independently reviewed and accepted by `/root/a09_reviewer` on review attempt 2 before F02 implementation begins.
- **Trigger:** F02 requires behavioral assertions in the new `tests/everyday/tst_page_factory.cpp`, but that file cannot execute unless it is added to the existing production `application_tests` target. `tests/everyday/CMakeLists.txt` is included after that target exists, but was absent from F02's original allowlist. The registered suite is necessary to verify T013–T015; leaving the test source unregistered would make the required application evidence impossible.
- **Change:** add `crates/ferric-browser-engine-qt/tests/everyday/CMakeLists.txt` as F02's eighth allowed product path and read anchor; add F02 to the existing source-map record; add the `test-registration` lock while retaining `build-registration`; keep test registration in the existing production-backed target.
- **Dependency/lock effect:** no dependency graph change. F02 still depends only on accepted S01. The extra named lock and allowed path serialize F02 with W00 and all other users of the shared CMake registration file.
- **Test effect:** T013–T015 and their expected behaviors are unchanged. The amendment only permits compiling/registering the F02 behavioral test file in the existing production application test binary; no test, security guard, default, or acceptance threshold is weakened. F02 remains within the eight-product-file cap.


## A14 — F02 uses the registered application test harness

- **Status:** independently reviewed and accepted by `/root/a09_reviewer` on review attempt 1 at `2026-10-24T16:19:37Z`; source implementation/build validation remain paused until the single build slot is released.
- **Trigger:** F02 attempt 1 uses the existing `tests/application/tst_application.cpp` and its registered `ApplicationTests` slots for T013–T015. The proposed `tests/everyday/tst_page_factory.cpp` is not needed; the added CTest registrations invoke the production `application_tests` target. The test source was missing from F02's edit allowlist, which otherwise prohibits the submitted test assertions.
- **Change:** replace F02's unused proposed test source with `tests/application/tst_application.cpp` in `allowed_paths` and `read_anchors`; remove unused edit paths `FerricBrowserWindow.qml`, `FerricPageServices.qml`, and `build.rs`, retaining them as read-only anchors. Keep the primary shell, two extracted QML components, the existing application test source and everyday CMake registration as F02's five allowed product paths. Remove the unused `tst_page_factory.cpp` proposal from the source map, add the baseline/application-blob record for the application harness and retain the A13 `test-registration` lock.
- **Dependency/lock effect:** no dependency graph change. F02 still depends only on accepted S01 and now has five allowed product paths, below the eight-file cap. Existing `page-factory`, `primary-shell`, `secondary-shell`, `build-registration` and `test-registration` locks remain.
- **Test effect:** T013–T015 and acceptance thresholds remain unchanged. The existing production harness exercises the same cold page creation, actual popup/transfer path and captured title-owner callbacks, and its commands remain registered as CTest cases in `tests/everyday/CMakeLists.txt`. This changes only where the required assertions live; it does not weaken assertions, security behavior, defaults or evidence requirements.


## A15 — Align W01 dependency metadata with the plan

- **Review gate:** independent acceptance is required before W01 dispatch; the decision is recorded at `evidence/A15-a1/PLAN_REVIEW.json`.
- **Trigger:** `control/plan.json` lists W01 dependencies as `F01b` and `S00`, while the worker-facing W01 packet and `SLICES.md` index list `F01` and `S00`. The scheduler reads `control/plan.json`, so the conflicting worker documents can misstate the accepted prerequisites.
- **Change:** update only `slices/W01.md` and `SLICES.md` to say `F01b, S00`. Leave the existing plan unchanged as the authoritative dependency record.
- **Dependency/lock effect:** no graph or lock change. W01 remains dependent on the already accepted F01b and S00 slices; F01b itself depends on F01 in the plan.
- **Test effect:** no product code, tests, or acceptance thresholds change. The amendment only makes the human-readable W01 prerequisite text match the scheduler.
