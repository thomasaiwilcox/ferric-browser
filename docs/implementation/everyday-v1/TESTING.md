# Verification recipes and evidence rules

## Before every command

Read the current source's build/test instructions and S00 environment map. Use a clean committed candidate for acceptance evidence. Worker-development tests may run before commit, but their receipts remain development observations. Evidence lives outside the product checkout; include source SHA/tree, compiler/Qt/KF6 versions and fixture conditions. Do not print credential values. Do not claim a source-string match, command existence or test registration is a behavioural pass.

The command wrapper does not invent a test. It captures a real process, source snapshots, exit status, timeout and log hash. `--require-tests` requires a recognizable nonzero test summary and no reported skips. A baseline suite with legitimate existing skips can be recorded without that flag, but those skipped tests do not satisfy a required acceptance case. Reviewers must check the actual case names and assertions.

Set these paths from RUN_CONTEXT, not a guessed home directory:

```sh
REPO=/absolute/path/to/repo
RUN=/absolute/path/to/external-control-workspace
PACK="$REPO/docs/implementation/everyday-v1"
```

Example wrapper (the `--` separates an argument array, not a shell script):

```sh
python3 "$PACK/tools/run_gate.py" --repo "$REPO"   --out "$RUN/evidence/F00-a1/config-tests" --label F00-config   --timeout 1800 --require-tests -- cargo test -p ferric-browser-config --locked
```

Every invocation needs a fresh evidence directory. A timeout is a failed/unqualified invocation even if partial test output looks green. Don't retain secrets in logs; test with disposable synthetic identities.

## Existing baseline gates

Verified in the inspected CI; recheck any renamed commands in S00:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --keep-going --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
cargo xtask test engine
cargo xtask test application-clipboard
cargo xtask docs
git diff --exit-code -- docs/user
desktop-file-validate packaging/io.github.ferricbrowser.FerricBrowser.desktop
```

Run the appropriate focused crate first, not the whole native suite after every line. `cargo xtask check` already includes substantial work; inspect its current task graph to avoid duplicate expensive runs. Capture preexisting failures before product changes and keep them visible. Existing guards around native SDK/source, portals, permissions, notifications, downloads, search, settings and recovery may not be disabled.

Pure crate selection: `ferric-browser-core`, `ferric-browser-config`, `ferric-browser-runtime`, `ferric-browser-application`, `ferric-browser-storage`, `ferric-browser-ipc` as applicable. A root workspace command may pull in Qt even if a pure subset does not.

## New production-backed everyday harness (introduced by S01)

The following paths/targets are **deliverables**, not preexisting commands at authoring. S01 creates their CMake registration before downstream tasks invoke them. Reuse the real release engine and shipped QML, current application/portal fixture utilities and the current compiled-resource manifest. Do not paste production implementations into a fake test browser.

```sh
NATIVE="$RUN/builds/native-S01-a1"
export CARGO_TARGET_DIR="$NATIVE/cargo-target"
cargo build --release --locked -p ferric-browser -p ferric-browser-engine-qt
cmake -S "$REPO/crates/ferric-browser-engine-qt/tests/application" \
  -B "$NATIVE/cmake" \
  -DFERRIC_ENGINE_LIBRARY="$CARGO_TARGET_DIR/release/libferric_browser_engine_qt.a"
cmake --build "$NATIVE/cmake" --parallel 2
ctest --test-dir "$NATIVE/cmake" --show-only=json-v1
ctest --test-dir "$NATIVE/cmake" --verbose --output-on-failure --no-tests=error

# Capture the S01 acceptance tests with visible QtTest totals/skips. The test
# command is the production-backed CTest filter; the run_gate guard rejects
# empty summaries and any reported skipped test.
python3 "$PACK/tools/run_gate.py" --repo "$REPO" \
  --out "$RUN/evidence/S01-a1/everyday-acceptance" \
  --label S01-everyday-acceptance --timeout 1800 --require-tests -- \
  ctest --test-dir "$NATIVE/cmake" --verbose --output-on-failure \
    --no-tests=error -R '^everyday_'
```

Configure the `tests/application` parent project: it creates `application_tests` and includes `tests/everyday` as an additive child, so configuring the child directory alone is invalid. Register and mark the native build root as RESOURCE_POLICY requires. If Cargo's target directory differs, pass its real absolute library path. Inspect CMake and the compile/link output to establish which library was linked. A native suite built from an older candidate does not qualify the current candidate.

S01's test inventory must map each acceptance ID to executable/test method, fixture and evidence level. Reject missing registrations and an empty `ctest -R` filter. Use `--no-tests=error` with CTest. Run the acceptance filter through `run_gate.py --require-tests` and verbose CTest so QtTest totals and skips are visible; any skipped required test fails that gate. A skipped test is not equivalent to a pass. Don't add placeholder tests just to get the inventory nonempty.

Current existing application tests are under `tests/application/` and configured by current xtask/application setup. Preserve suites including page_search, hint_controls (including DPR2), navigation_usability/recovery, privacy_identity, switcher, close_requests, tab_recovery, last_tab_undo, open_targets, form_reload/streaming_history, downloads and startup/session recovery. Obtain exact test names via CTest inventory rather than hardcoding the whole old CI regex into a new pack.

## Recipe catalogue

| Recipe named in slice | Required execution |
|---|---|
| `preflight` | Run preflight.py; inspect actual checkout/remotes/instructions, source and SDK drift. Review source-map.execution.json and clone safety observations. No runtime claim. |
| `baseline` | Register S01 harness; run current pure and relevant production-host baseline; archive discovery list and environment gaps. |
| `config` | Config crate parsing/default/layering/registry tests; application configuration tests; generated-doc diff; real Settings apply/pending/restart where changed. |
| `rust-policy` | Focused core/engine-pure policy tests with deterministic events/clocks; positive and invalid transition cases; formatting/Clippy. Some engine tests still require Qt linkage. |
| `application-rust` | Application/runtime tests for captured effects, settings/presentation routing and session scope; no GUI logic copied into Rust mocks. |
| `storage` | Storage/application migration and round-trip tests with disposable roots; old-format fixtures, corruption/interruption and private nonpersistence. |
| `application` | New production host suites and relevant existing regressions; real WebEngine callbacks, focus, user inputs and timers. Test in both presentations/windows where relevant. |
| `credential` | P00 isolated-world proof or subsequent actual broker/helper/JS tests: main/subframe authority, origin/PSL, document swaps, binary options, native coexistence, abort/lifetime and no secret logs. Use a fake helper only for specified fault injection, not as positive phone-transport evidence. |
| `helper` | Separate helper fmt/Clippy/test/build with its own lock, protocol bounds, PSL validation and candidate library calls. Audit actual deps/toolchain/license. |
| `rp` | Independently verifying RP fixture; registration followed by assertion verification, mismatch and UV/algorithm rejection; cross-check known-good browser only with disposable credentials. |
| `fuzz` | Bounded reproducible parser/transition corpus plus fuzz smoke on the new surfaces. Record engine, seed, corpus, duration and findings. Never count an empty fuzz corpus or merely compiling a target as fuzz execution. |
| `benchmark` | Real release hot/cold measurements, identical workloads, raw samples/failures, >=10 warmups and >=100 measured samples/condition, PSS/CPU and 1,000-cycle lifecycle. Native performance qualification is separate from offscreen instrumentation. |
| `native` | Actual isolated Wayland/Hyprland process with current native bindings. Record compositor/GPU/Qt/input method. Exact target/group/focus/restart evidence; no success from env-variable detection alone. |
| `physical` | Real phone and native authenticator journeys using approved disposable RP/accounts; registration + sign-in + independent verification, cancellation/failure and optional availability. Record operator/device/OS/provider and sanitized outcomes. |
| `package` | Build/install/uninstall within disposable staging/prefix, helper executable discovery, licenses/PSL/runtime deps, missing-helper path. Do not install over the user's browser. |
| `ci` | Exercise registered gates from the production builds, plus feature-off default; missing test/environment reports preserved. Publishing workflow changes needs only local edits, not remote dispatch authorization. |
| `docs` | Generate current registries and check clean regeneration. Validate examples against actual config/commands; clearly distinguish implemented vs qualified. |
| `review` | Independent actual diff/control-flow/security review and rerun affected integrated-source tests; concrete findings/report, not author self-approval. |
| `rebuild` | Safe owned cleanup, exact-source rebuild, fresh test runner provenance, repeat affected tests. |
| `release` | Strict ledger validation, final exact-source positive qualification and truthful local handover; no unqualified feature promoted. |

## Separate helper commands

After P01 creates its real workspace/lockfile:

```sh
cargo fmt --manifest-path helpers/ferric-phone-auth/Cargo.toml -- --check
cargo clippy --manifest-path helpers/ferric-phone-auth/Cargo.toml       --all-targets --locked -- -D warnings
cargo test --manifest-path helpers/ferric-phone-auth/Cargo.toml --locked
cargo build --manifest-path helpers/ferric-phone-auth/Cargo.toml --release --locked
```

Use its explicitly qualified compiler, not a silent root toolchain update. Root `cargo test --workspace` does not test an excluded helper workspace; X02 must register both. Pin any independent RP verifier and test tooling separately from product dependencies; use an existing reputable verification implementation instead of custom crypto. P13 records actual selected version and lock.

## Native and physical test matrix minimum

Internal tabs and windows presentation: two simultaneous normal profiles, private and ephemeral sessions, top-level auth popup, group/standalone windows, original visible host closure, compositor restart, ordinary browser shutdown, active capture/download and pending prompt. Include same-URL reload, SPA change, BFCache traversal, renderer termination, view replacement and rapid key repeats.

Passkey positive tests require both iPhone and Android, separately reported; one device cannot stand in for the other. Test at least one actual native security key after the chooser/shim is installed. Do not register real personal accounts or request biometric/PIN secrets in agent transcripts. A human records approved physical steps; agents prepare fixture/code/logs and leave blocked records when equipment is absent. The capability can be implemented but cannot be described as fully qualified.

Performance: measure internal and window spare variants independently; a failure to improve or excessive memory means hot defaults remain off with the result reported. Do not hide failed/slow samples. The acceptance thresholds are chosen product budgets in C03, not claims about current hardware.

## Evidence format and checker limits

Use templates/EVIDENCE.json for one slice: every case maps to a concrete assertion and a hashed artifact. Command receipts are created by run_gate.py; review records use REVIEW.json. Native/physical/benchmark observations also need QUALIFICATION_RECORD.json with actual environment and observation details. All source hashes must refer to the code exercised, not the pack author's source.

validate_pack.py checks structure, references, hash consistency, independence and declared evidence levels. It cannot prove an operator actually scanned a QR or a test is correctly designed. Independent review remains mandatory. Initial files intentionally contain no success evidence; strict validation must fail on an unstarted run.

## A37 replacement evidence

Historical T014c/T014d map to T014e1/T014e8 at the same component/application evidence levels and EF-F02 requirement. The new native T014e0 is a feasibility gate; old receipts do not qualify it. T014a/T013/T015 behavior remains mandatory on the final production host. T045 now requires native measurement on this machine only, retaining all C03 thresholds and raw-sample requirements. No physical/authentication gate is waived.
