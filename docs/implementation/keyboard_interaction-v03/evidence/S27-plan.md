# S27 integrated audit plan

## Scope

Audit the integrated checkout after S26, rerun the C22 repository gates, inspect source-to-contract and invariant coverage, and record a truthful release qualification result. Do not install or publish the browser, open private profiles, or claim live-site support without an authorised disposable profile and an available input seat.

The audit found strict Clippy failures in the S25-only ignored label probe. The probe now uses checked conversions and `clone_from`; a post-fix rerun produced output semantically identical to S25. This is the sole source change made during S27 and has no production behavior or default effect.

## Gate order and evidence

| Gate | Command | Evidence |
| --- | --- | --- |
| Format | `cargo fmt --all -- --check` | `S27-fmt.log` and the passing `cargo xtask check` rerun |
| Strict Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | `S27-clippy.log`; initial findings retained in `S27-clippy-initial-failure.log` and `S27-clippy-intermediate-failure.log` |
| Workspace tests | `cargo test --workspace --locked` | `S27-workspace-tests.log` |
| Generated contracts and deterministic project check | `QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask check` | `S27-xtask-check.log`; the first invocation without forced GC is retained separately |
| Qt/WebEngine and native CTest | `QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc cargo xtask test engine` | `S27-engine-native.log` and verbose `S27-native-qt-offscreen.log` |
| Adapter smoke | `cargo xtask test adapter` | `S27-adapter-smoke.log` |
| Nested Wayland startup/input availability | `cargo xtask test wayland` | `S27-wayland-smoke.log` |
| Changed opt-in probe | `CARGO_NET_OFFLINE=true cargo test -p ferric-browser-engine-qt --test label_probes -- --ignored --nocapture` | `S27-label-probe.log`, compared with `S25-label-probe.log` |

The integrated source commit assessed by the gates is `7dbfb14af070c2494077ca6342ad009cefb41840`. Reports and the final evidence ledger are committed afterward without changing tested source.

## Release boundary

Physical Wayland input cannot be qualified in this nested compositor because its virtual-keyboard protocol is unavailable. No authorised live account/profile or installed Ferric release was present. S20 therefore remains `blocked_environment`, S21 remains `blocked_environment` without an implementation, and the S21 `not_applicable` wording is not applied because the pack evidence contract and validator require an evidenced S20 `research_no_go` first.

S25 performance measurements remain the latest runtime measurements because S26 and S27 did not change production code. Their callback and overlay threshold misses remain release risks.
