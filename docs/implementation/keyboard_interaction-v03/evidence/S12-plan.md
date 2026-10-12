# S12 implementation outline and evidence map

## Intended changes

- `crates/ferric-browser-config/src/selector_config.rs` and
  `crates/ferric-browser-config/src/validation.rs`: keep the 16-selector and
  16 KiB bounds on each authored entry/rule, while permitting up to 32
  deduplicated selectors and 32 KiB in a resolved family. Resolve built-ins,
  active global/profile/runtime policy, then matching site rules in the
  existing priority/source order. Return effective selectors and bounded
  provenance for all seven families.
- `crates/ferric-browser-config/src/tests.rs`: cover empty replace, empty
  extend, stable per-family layering, site-rule order, all-family independence,
  per-entry versus resolved count limits, per-rule aggregate bytes, and the
  32 KiB resolved-family bound.
- `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/src/browser_ui_navigation.rs`, and
  `crates/ferric-browser-engine-qt/src/browser_ui_configuration.rs`: expose a
  browser-owned effective-policy snapshot for the active family/site with the
  current hint configuration revision; reject collection results from a stale
  revision and retain the revision/provenance with the hint session. Preserve
  existing configuration-change cancellation and navigation/site-rule order.
- `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimePresentation.qml`
  and `crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js`:
  pass the resolved policies into discovery; apply the requested family's
  selectors and the explicit `scrollables` policy to the implicit all-family
  scrollable pass; capture policy/revision on retained element records; apply
  that captured policy at fresh, pointer-preflight, focus, and final-click
  checks while keeping the existing semantic action and proxy checks.
  Bound a snapshot to 50,000 inspected elements and 5,000 returned candidates,
  report visible truncation, and avoid claiming a synchronous selector match
  can be interrupted.
- `crates/ferric-browser-engine-qt/tests/qml/tst_webapp_interaction.qml` and
  config/engine unit tests: add production-script WebEngine coverage for
  replacement/extension, custom discovery through fresh/final checks, disabled
  implicit scrollables, invalid-kind/action denial, revision mismatch,
  truncation, and the existing selector parser/commit safeguards.
- `crates/xtask/src/main.rs` and the generated `docs/user/configuration.md`:
  document seven-family selector scope, precedence, limits, empty replacement,
  discovery-only authority, resource caps, and visible truncation. The xtask
  source is included because hand-editing the generated output is discarded by
  `cargo xtask docs` and rejected by `cargo xtask check`.

## Required test-to-path map

| Test | Planned production evidence |
| --- | --- |
| T009 | Existing strict `hint_payload.rs` parser tests plus `tst_webapp_interaction.qml` privacy/label regression cases. |
| T030 | Existing selector parser and callback/commit tests, extended for invalid effective layering and stale policy revision; WebEngine selector syntax fixture. |
| T031 | Config resolver tests and production `hintCollector` cases for extend/replace, empty replace, site precedence, independent `all`, and count/byte boundaries. |
| T032 | Production WebEngine fixture proving a custom-selected semantic button survives fresh and final click checks, while invalid kinds cannot gain click/focus/proxy/native authority; stale revision is rejected. |
| T033 | Production WebEngine stress fixture asserts no more than 50,000 inspected elements and 5,000 output candidates and reports `truncated`; record that individual CSS matching is synchronous. |
| T045 | Full `cargo xtask check --locked --offline`, `cargo xtask test engine`, and `cargo xtask docs` gates; engine test includes Qt Quick/WebEngine and native CTest coverage. |
| T055 | Generated docs and complete generated-contract check, including config registry, CLI/IPC parity, and clean regeneration. |

S12 remains `implemented_unqualified` unless the real composed BrowserUi
configuration/session journey and the required environment evidence qualify
the complete path. No toolchain, dependency, version, or branch changes are
planned.

## Actual implementation notes

- `validate(config)` now delegates each global site rule to the shared
  `validate_site_rule` path. This closes a config parity gap found during S12:
  the dedicated site-rule transaction accepted selector keys, while full
  config validation rejected them. Authored entries remain capped at 16
  selectors; layered families may reach 32, and one site rule shares a 16 KiB
  string budget across its families.
- The effective snapshot is resolved from BrowserUi's active config and current
  URL, includes all seven family policies plus provenance, and is hashed with
  the active configuration revision and family. QML passes the snapshot as an
  opaque string into the production script builder. The retained DOM records
  capture selector lists/revision/provenance; Rust sessions retain the numeric
  revision and selected-family provenance.
- Collection now uses a shared 50,000-node TreeWalker budget across the
  document, open shadow roots, same-origin frames, preferred-link checks, and
  proxy candidate discovery, with a 5,000-candidate ceiling. Proxy pairs are
  mapped during the same traversal. Truncation is surfaced in the Hint status.
  The T033 stress fixture exercises 32 selectors, including 31 relational
  `:has()` selectors, against the capped document.
- The opt-in renderer benchmark recorded 10 warm-ups and 100 samples for six
  scenarios: 500 candidates at 80%, 100%, and 125%; 1,000 and 5,000 candidates;
  and a 1,000-candidate mutation case. Raw samples and nearest-rank summaries
  are in `logs/S12/performance.log` and `logs/S12/performance-summary.json`.
  These are current-build renderer timings, not a same-build before/after
  comparison or full BrowserUi/transfer/overlay qualification.

## Qualification gaps

- No composed offscreen BrowserUi journey drives startup validation, site-rule
  selection, config commit/cancellation, policy staleness, and final action
  through the real configuration lifecycle. Unit and production-script fixture
  evidence cover their respective boundaries, but do not prove the whole
  lifecycle together.
- No physical Wayland keyboard journey was run. The QML/WebEngine and native
  CTest results are offscreen evidence only.
