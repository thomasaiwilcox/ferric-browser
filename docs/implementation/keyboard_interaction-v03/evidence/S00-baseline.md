# S00 baseline and authority inventory

Implementation starts from `e9a576fd375927733b200f91a6387dea4698840d` on
`implementation/keyboard-interaction-v03`. The pinned contract baseline is
`4337056f632de0c710943fa6df76c67cf14b8b47`. This checkout includes the subsequent
native start page, guide, and history changes (77 files; see `logs/S00/baseline-diff.log`).
The separate `feature/versioned-browsing` branch remains at `08cc2bb`.
The initial untracked pack, root `tools/`, and `aqtinstall.log` are preserved.
The pack's actual directory spelling is `keyboard_interaction-v03`; references
in evidence use that existing location. Normative files are not renamed or edited.

## Intended patch and verification mapping

S00 changes only this implementation ledger and evidence. T001 inspects the
actual source authorities and installed environment. T045 runs workspace tests
(including the CLI), existing QML/WebEngine regressions, and the native routing
suite. The whole repository gate checks generated docs, architecture, QML lint,
packaging/resource integration, formatting, and Rust tests. The adapter and
nested Wayland smoke use disposable profiles and do not qualify live websites.
Results and exact commands are in `logs/S00/gate-results.json` and `S00.json`.

## Current symbols and contract ownership

| Boundary | Current source and actual symbols | Contract implications |
| --- | --- | --- |
| Target identity | `core/src/hints.rs`: `HintTarget`, `HintSession`, `HintInteraction`; `engine-qt/src/browser_ui_navigation.rs`: `HintSession::new`, selection/refresh | Reuse tab/generation/document/session identity; no second registry or text ranker. |
| DOM identity | `qml/scripts/BrowserScripts.js`: `hintCollector`, `hintFresh`, `hintFocus`, `hintClick`, `hintStopTracking`; `__ferric_browserHintState.weak/elements` | Retained DOM objects already exist. Collector/fresh descend open roots; final actions currently do not. Reproduce before repair. |
| Hint grammar and facts | `engine-qt/src/hint_policy.rs`, `hint_payload.rs`; core command registry; IPC `command_invocation.rs` | Extend closed typed policy and bounded parser, preserving all frontends. |
| Local invocation | `browser_ui_input.rs`, `browser_ui_ui_actions.rs`, `browser_ui_queries.rs`, `browser_ui_command_dispatch.rs`; `CommandSource`, `CommandInvocation`, `DispatchTarget` | Source enum alone is not a physical-input permit. Macro maps to UI availability today; S03 must prevent laundering. |
| Native key ownership | `browser_key_router.h/.cpp`: `FerricBrowserKeyRouter::eventFilter`, `acceptCurrentEvent`, `inputEpoch`, `forwardedKeys_`, entry/accepted keys | Preserve original Qt events, pending gestures, IME, recovery and native recipient focus. |
| Native pointer | `browser_page_pointer_adapter.h/.cpp`: `FerricPagePointerAdapter::dispatch`, `surfaceSerial`, `surfaceRevision`, `inputBlocked`, request deduplication/watchdog | One adapter, one transport; explicit acknowledgements; no claimed DOM/native atomicity. |
| Grid | `browser_ui_spatial.rs`: `begin_spatial_navigation`, projections and cancellation; core `SpatialSession` | Current Grid is local and macro-restricted. C05 requires a narrow recorded ADR amendment, not generic coordinate injection. |
| Scroll | `BrowserScripts.js`: `scrollTargetRuntime`, `ftClippedRect`, `ftReachableFromTopDocument`, `hintSetScrollTarget` | Exact weak Element policy is independent of focus. Preserve viewport overflow propagation. |
| Final callback | `FerricBrowserRuntimePresentation.qml`: `activateHint` calls fresh selection, then final focus/click, then closes Hints in callback | Must audit owner/session lifetime and map teardown before adding effects. Existing callback strings are not proof of security. |
| Configuration | `browser_ui_configuration.rs`: `commit_runtime_config`, `execute_config_reload_command`; application `update_configuration`/configuration boundary | Existing atomic Rust activation is present; full CSS validation and asynchronous revision guards are new S11 work. |
| Script world | `FerricBrowserRuntimeBase.qml`: `browserScriptWorld = WebEngineScript.ApplicationWorld`; services installs browser scripts in this world | MainWorld userscripts are separate. No privileged page bridge. |
| Resources | `engine-qt/build.rs`: `collect_browser_resources`, `write_script_manifest`, `write_manifest_qrc`; manifest SHA-256 and bundle version 1; script VERSION 13 | Register extracted JS through the existing resource/hash mechanism. Preserve version parity assertions. |
| Generated contracts | `xtask/src/main.rs`: `check_generated_docs`, `check_architecture_boundaries`; registry-backed docs | Coordinator owns generated docs, resource registration, runner changes. |

All abbreviated paths in this table are under `crates/ferric-browser-*`.
Compared with the pinned baseline, `BrowserScripts.js` adds the ApplicationWorld
history document token/probe and advances VERSION 12 to 13; its Hint implementations
are unchanged. `BindingResolver::is_pending` now exists and default tab/start page
bindings have changed. Retain these later accepted changes.

## Real suite entry points

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
cargo xtask check --locked --offline
cargo xtask test engine
cargo xtask test adapter
cargo xtask test wayland
git diff --check
```

`cargo xtask test engine` runs engine Rust tests, configures/builds
`target/qml-tests/qml_tests`, then executes the full QML suite with
`QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME= QT_QUICK_BACKEND=software`,
then configures/builds `target/native-insert` and runs `ctest --test-dir
target/native-insert --output-on-failure`. Native CTest additionally sets
`QTWEBENGINE_CHROMIUM_FLAGS=--disable-gpu`. QML WebEngine tests are renderer
behaviour evidence; native QTest events are offscreen input evidence.
`cargo xtask check` does not run native-insert; engine is a separate required gate.
The check task contains generated-doc comparison and resource/build checks;
there is no standalone invented resource test command.

Qt 6.11.2, Rust/Cargo 1.98.0, CMake 4.4.3, Weston and D-Bus are installed.
`qmllint` is installed at `/usr/lib/qt6/bin/qmllint` and discovered by xtask.
An installed public-API probe reports QtWebEngine 6.11.2 and Chromium
140.0.7339.225 (`logs/S00/engine-version.log`); the fixture user-agent reports
the expected coarse Chromium 140.0.0.0. Wayland startup does not
prove a keyboard seat, physical routing, compositor scale, IME, or live-site
behaviour. No private account or profile is an authorised fixture.

## Fixture provenance and scheduling

Use `tests/fixtures/webapp-interaction/` as the canonical fixture root.
Fixtures are local, self-contained, synthetic, and contain only fixed test
labels and effect counters. Framework emulations must say ACE-like or
Bootstrap-like. Any real vendored asset needs version/hash/licence metadata.
No live network or account is required by deterministic CI.

Maintain the existing immutable TEST_MATRIX identities. Store raw durations,
commands and outputs under `evidence/logs/Sxx/`; label evidence separately as
pure-unit, qml-component, webengine-fixture, native-qt-offscreen,
native-wayland, live-site, performance, or static-inspection. Do not infer one
class from another. Planned positive assertions can legitimately fail at
baseline; record the red result without weakening the assertion.

Use serial ascending ready slices. `plan.json` owns production lock groups;
the current full inventory is in that file. Shared DOM runtime, Hint schemas,
native routers/adapters, config commits, runner/resource changes and generated
registries remain coordinator-owned. No agents are delegated. Only implemented
and tested contract dependencies unlock a production feature. Missing native
qualification remains explicit and cannot enable a gated capability.

## Adversarial self-review

S00 grants no new page/input/config capability. Logs inspect only synthetic
test state and repository/environment metadata. Existing source reveals the
shadow final-action mismatch, optimistic focus success, frame-border and
composed-slot risks; these remain hypotheses until S01 fixtures run. Session
owner, exact element, revision, paired identity, effect cardinality, teardown
and privacy will be tested in their assigned slices. No independent reviewer
has been used. No toolchain, dependency, lockfile, personal profile, daily
browser or system configuration has been changed.
