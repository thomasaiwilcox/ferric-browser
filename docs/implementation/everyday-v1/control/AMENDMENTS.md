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
