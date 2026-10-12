# S13 implementation outline and evidence map

## Intended changes

- `crates/ferric-browser-engine-qt/src/hint_policy.rs`,
  `crates/ferric-browser-ipc/src/command_codec_content.rs`, and
  `crates/ferric-browser-ipc/src/command_argument_validation.rs`: accept the
  explicit `hint regions` and `hint surfaces` families in the same command
  grammar as existing Hint families. Keep them outside `hints.selectors`;
  reject rapid, pointer, and URL-action combinations for these family-only
  palettes. Do not add a key binding, so `f` continues to select `all`.
- `crates/ferric-browser-core/src/hints.rs` and
  `crates/ferric-browser-engine-qt/src/hint_payload.rs`: add closed typed
  region/surface candidate kinds and a Rust-owned capability table. The page
  payload carries only bounded candidate facts; no page field can request an
  action. Region actions are Hint/Search/Grid within, with Set scroll target
  only for an independently scrollable region. Surface actions are Grid
  within, with explicit Focus only for a freshly verified focusable surface.
- `crates/ferric-browser-engine-qt/src/browser_ui_navigation.rs`,
  `browser_ui_hint_selection.rs`, and
  `qml/components/FerricBrowserRuntimePresentation.qml`: bind the special
  family to the current selector/config revision without treating it as a
  configurable CSS-selector family. Selecting a region or surface opens its
  Rust-described action palette; ordinary families retain their existing
  selection behavior. Unsupported scope actions remain unavailable until
  their dependent slices implement them.
- `qml/scripts/PageInteractionScripts.js` and, only if required,
  `qml/scripts/BrowserScripts.js`: use the existing retained-element WeakMap,
  owner document, frame path, visibility/clipping helpers, and candidate map.
  Collect only explicit structural/named regions and visible surface elements;
  share eligibility with ordinary candidates; enforce the 256-per-family cap,
  50,000 inspected-node budget, and 5,000 overall bound. Apply the 16-by-16
  CSS-pixel threshold only to unnamed visual surfaces. Do not broaden the
  default `all` collector or infer semantics from generic-div area.
- Existing WebEngine fixture/test locations:
  `crates/ferric-browser-engine-qt/tests/qml/tst_webapp_interaction.qml` and
  `tests/fixtures/webapp-interaction/`; add deterministic region, surface,
  hidden-buffer, flood, cross-origin exterior, and closed-shadow-root cases.
  Add Rust/IPC grammar and capability-table tests beside their existing tests.
- `crates/xtask/src/main.rs` and generated `docs/user/` output: document the
  explicit families, caps, labels, action availability, and cross-origin
  limits. `TEST_MATRIX.json` and `evidence/S13.json` record exact outcomes.

## Required test-to-path map

| Test | Planned production evidence |
| --- | --- |
| T009 | Existing strict payload parser plus new region/surface payload cases for typed kinds, bounds, and rejection of extra fields; WebEngine labels remain visible text only. |
| T017 | Existing proxy regression suite remains green; region/editor recognition cannot grant a proxy receiver exception or introduce a paired identity. |
| T034 | Production `hintCollector("regions")` and `hintCollector("surfaces")` fixtures prove structural/named classification, identity deduplication, per-family cap, no generic-div flood, hidden/offscreen canvas exclusion, and unchanged `all`. |
| T035 | Production WebEngine fixtures prove cross-origin iframe exterior only and no closed-root traversal. Run raw native Grid only if an existing real native Wayland journey can exercise it; otherwise mark that required level unqualified. |
| T045 | `cargo xtask test engine` plus the full locked/offline xtask check cover legacy behavior, QML, IPC, and native CTest as available. |
| T055 | Full xtask check and generated docs verify CLI/IPC/registry/help parity and reject incompatible special-family arguments. |

S12's explicit unqualified findings are retained: there is no composed
BrowserUi configuration/session acceptance journey and no comparable previous
renderer baseline. Its selector and stale-target security tests passed, so the
dependency is implemented for this bounded extension. No selector-config
families, new default binding, second identity registry, or cross-origin child
authority are planned. No toolchain, dependency, version, or branch changes
are planned.

## Scope amendment

`qml/components/FerricBrowserRuntimeRequests.qml` and
`qml/components/FerricContextMenu.qml` were added to the write set because the
new palette includes intentionally disabled future actions. The shared menu
item builder must carry the enabled flag, and mnemonic activation must honor it
as well as the visual MenuItem state; otherwise a disabled action could still
be invoked from a keyboard shortcut.

## Qualification limits to preserve

- T035's `native-wayland` level remains unqualified if a physical keyboard
  journey is unavailable; offscreen/native CTest cannot substitute for it.
- A production fixture proves the browser-owned script boundary, not a full
  composed BrowserUi capability handoff. Do not describe it as end-to-end
  scoped-action qualification before S14/S15.
