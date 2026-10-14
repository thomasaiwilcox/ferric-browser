# S05 implementation outline and test map

## Baseline and bounded design

The pinned baseline has no `hints.proxy_targets` setting, paired candidate type,
or visible-representative-to-receiver path. This is unsupported-at-baseline new
work, not a reproduced red defect. Current `BrowserScripts.js` and Rust hint
session/payload code are the production anchors; the new behavior will reuse
their existing collector, retained element map, configuration revision,
selection, focus observer, and final-action path.

The repository's 1,000-line module guard required extracting the new effective
setting projection into `crates/ferric-browser-engine-qt/src/browser_ui_hint_proxy_configuration.rs`.
That sibling module contains only the `hints.proxy_targets` effective-value
projection to the existing QObject property and captured Rust state; the
configuration lifecycle and invalidation remain in their existing owner.

Add a typed `hints.proxy_targets = "off" | "validated"` setting with `off` as
the default. Preserve global/profile layering and apply matching site-rule
overrides using existing precedence. Rust must reject paired candidate kinds
while the effective captured setting is off.

Extend the existing Hint candidate with one optional, bounded associated
element ID. Only `EditorFocus` and `LabelControl` may carry it; all other kinds
must omit it. The browser-owned script retains both exact DOM objects in the
existing per-document map. Fresh selection and final action re-resolve the
representative and the exact original associated object, and reject either
object being detached, replaced, adopted, disabled, inert, or otherwise
ineligible. Activation must never find and substitute a new receiver.

`EditorFocus` is limited to a visible ACE-like container with the expected
container role/structure and exactly one descendant text proxy. The proxy may
be transparent, small, or pointerless, but must remain connected, same-document,
enabled, non-inert, editable, and not hidden by `display:none` or hidden
visibility. Focus the retained proxy and verify the settled deep active element.
Do not change page CSS, tabindex, disabled state, or Insert mode.

`LabelControl` is separate: use the exact visible native label and its
`label.control` association to an enabled checkbox/radio. Dedupe it when the
associated control is already a normally usable visible Hint target. Final
activation uses the retained label's ordinary semantics once and verifies the
associated control's resulting checked state in fixtures. It does not use the
editor focus exception.

Labels use bounded accessible names, associated label/title, or visible
representative text; never control values or hidden editor buffers. Selector
configuration cannot create either pairing or bypass the off setting.

## Production-path test map

| Test | Production path and assertion | Evidence class |
| --- | --- | --- |
| T015 | `hints.proxy_targets` resolution → `hintCollector` → typed Rust candidate parser/session → pair-aware `hintFresh` → `hintFocus`; exact unique proxy receives focus; class-only and ambiguous signatures reject. | WebEngine fixture; native Qt offscreen |
| T016 | Opt-in collector → native `label.control` relation → exact-pair freshness → one ordinary label activation; checkbox/radio toggles once; disabled, inert, and replacement reject. | WebEngine fixture; native Qt offscreen |
| T017 | Off setting and adversarial receiver types/ancestry/replacement/document changes → no proxy exception; site selectors cannot grant it. | WebEngine fixture |
| T018 | Existing `FerricBrowserKeyRouter` and native WebEngine focus proxy receive real QTest key press/release and `QInputMethodEvent`; verify transient/locked Insert, Enter/Shift+Enter, Escape, lost-release handling, and composition. | Native Qt offscreen; native Wayland separately |
| T020 | Retained Hint map remains alive through accepted page callback and teardown releases records/observers once; distinguish the existing direct teardown test from full handoff proof. | WebEngine fixture |
| T045 | Existing Rust workspace, complete QML/WebEngine suite, and native engine fixture. | Pure unit; WebEngine fixture; native Qt offscreen |
| T055 | Typed setting registry/config/runtime/site validation, generated user documentation, and resource manifest parity. | Pure unit |

Recorded commands from S00: `cargo xtask check --locked --offline`,
`cargo xtask test engine`, `cargo xtask test adapter`, and
`cargo xtask test wayland`. The Wayland command previously proved only
startup smoke because the available Weston lacks the virtual-keyboard
protocol; it is not physical keyboard or IME evidence. Formatting and lint
commands are `cargo fmt --all -- --check` and
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`.

## Exit conditions and known limits

The setting remains off by default. Positive ACE-like and label fixtures pass
only with opt-in, the exact retained pair is revalidated, ordinary visible
inputs remain deduplicated, and private control/editor contents never become
Hint labels. Actual framework compatibility, platform IMEs, physical Wayland
typing, and the complete T020 destination-lease transfer remain separate
claims and will be recorded as unqualified if not proven.
