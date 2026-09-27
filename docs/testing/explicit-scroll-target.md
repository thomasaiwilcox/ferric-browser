# Explicit scroll target qualification

Date: 2026-09-27

The durable design and ownership decisions are recorded in
[`ADR-0014`](../architecture/ADR-0014-explicit-scroll-target.md).

## Runtime evidence

The deterministic local fixture is
[`fixtures/scroll-targets.html`](fixtures/scroll-targets.html). Page-script
qualification used Chromium 152.0.7977.82 with a 1400 × 1600 CSS-pixel
viewport. The generated expressions were built from
`BrowserScripts.js` and evaluated against the fixture through Chromium's local
DevTools protocol. No remote content or network resources were used.

This exercises the page-world collector, fresh validation, assignment, weak
policy state, and scroll scripts against a live DOM. A host-Wayland run also
drove the native Ferric window with `wtype` against the same fixture. That run
covered `;s`, `;S`, `j`, and `:scroll-target status`, including the transient
status-bar feedback used when the configured status bar is otherwise hidden.

| Case | Result | Evidence |
| --- | --- | --- |
| No selection, focused child in vertical pane, `j` | Pass | Automatic resolution moved pane A only. |
| Invoke the actual `;s` binding, select other vertical pane, then press `j` | Pass (native host) | Native hint `s` selected pane A. The following `j` moved pane A from 0 to 40 while pane B and the document remained at 0. A transient `Scroll target: selected region` notice was visible. |
| Invoke the actual `;S` binding after an element or document selection | Pass (native host) | The native binding restored automatic policy and displayed `Scroll target: automatic` even with `ui.statusbar = "in-mode"`. |
| Same selection, `h` | Pass | Returned `unsupported-axis`; no region moved and status remained element. |
| Same selection at bottom, `j` | Pass | Scroll position stayed at the bottom and the element policy remained. |
| Select horizontal pane, `l` | Pass | Only the horizontal pane moved. |
| Select two-axis pane, mix `j/l/Ctrl+d/G` | Pass | Element-local incremental, half-page, and edge scripts moved the selected pane. |
| Selected pane with nonzero `scrollLeft`, `gg`/`G` | Pass | Both vertical edge operations preserved the pane's horizontal offset. |
| Focus input A, select pane B | Pass | `document.activeElement` remained `input-a`; assignment did not call focus. |
| Clear the Hint map, force Chromium GC twice, then `j` | Pass (live QtWebEngine) | The still-connected weak target remained dereferenceable. Pane A moved from 0 to 40 while pane B and the document remained at 0, and policy stayed `element`. |
| Remove selected pane, `j` | Pass | Policy changed to automatic and the same operation moved the focus-resolved pane A. |
| Hide selected pane, `j` | Pass | Same-operation automatic fallback moved pane A. |
| Remove all overflow, `j` | Pass | Same-operation automatic fallback moved pane A. |
| Remove vertical but retain horizontal overflow, `j` | Pass | Vertical operation was a no-op; element policy remained selected. |
| Then press `l` | Pass | The retained element moved horizontally. |
| `scroll-target document` with focus in pane | Pass | The document moved while pane A stayed at its prior position. |
| `scroll-target auto` | Pass | Focus-aware pane scrolling resumed; page status object reported `auto`. |
| `scroll-target status` in every state | Pass (runtime + native selected-region check) | Page status objects were checked for element, document, automatic, and invalidated-to-automatic states. The native `:scroll-target status` command displayed the selected-region notice; source-boundary assertions cover the exact selected region/document/automatic mapping. |
| `scroll-target document ;; tab-next` | Pass (adapter regression + native host) | The queued action carries tab A's stable ID; QML resolved that ID after tab B became active. Native inspection reported A=`document`, B=`auto`. Crossed `document`/`auto` chains then reported A=`auto`, B=`document`. |
| `scroll-target status ;; tab-next` with different tab policies | Pass (adapter regression + native host) | The command issued from document-policy tab B, switched to automatic-policy tab A, and still displayed `Scroll target: document`. |
| Policy action after its issuing tab has no view | Pass (source boundary) | Stable-ID resolution yields no view and reaches the visible `Scroll target unavailable: no active page` path before the generic active-view guard. |
| Select same-origin frame/shadow target | Pass | The exact shadow pane and `srcdoc` frame pane each moved. |
| Select frame pane, replace its same-origin document, then press `j` | Pass | Old frame target invalidated; the same scroll operation returned automatic. |
| After remove, hide, overflow-loss, adoption, or frame invalidation, check status | Pass | Each invalid target reported automatic after invalidation. |
| Reload after selection | Pass (page-world runtime) | The generated scrollables collector and frame-aware setter selected pane B; after reload, a fresh isolated world reported automatic. Native Ferric reload/key dispatch was not driven. |
| Navigate after selection | Pass (page-world runtime) | After selecting pane B, top-level navigation to the same deterministic fixture with a query string created a fresh isolated world whose status was automatic. Native Ferric navigation/key dispatch was not driven. |
| Switch tabs | Pass (page-world runtime + native command chain) | Two independent Chromium page targets held separate policies. Native `tab-next` chains preserved the issuing tab's policy action while activating the other tab. |
| 100 paced native `j` inputs with an explicit target | Pass (native host performance watch) | All inputs completed in 2.14 seconds including a configured 15 ms per-key injection delay. Pane A reached its exact 470 px maximum, pane B/document remained at 0, policy stayed `element`, and the renderer immediately answered a follow-up probe. |
| Restore a session | Pass by construction + fresh-world runtime | `SessionTab` persists URL, pinned/muted, zoom, and document scroll position only; restore creates tabs and navigates them. The generated runtime test verified a fresh document world starts automatic. A Ferric-native saved-session restore UI workflow was not driven. |

Cross-origin frame contents remain excluded: collection and exact element
validation only traverse same-origin frame documents. No cross-origin claim is
made by this qualification.

The lifecycle checks above were performed in a temporary headless Chromium
152.0.7977.82 profile by evaluating the generated `BrowserScripts.js`
expressions through DevTools in a named isolated world. The assignment used a
candidate returned by the actual `hintCollector("scrollables")` builder and
passed its `element_id` and `frame_path` to `hintSetScrollTarget()`; it did not
call a replacement DOM selector or retain an element in the harness. These
checks qualify page-world policy lifecycle and tab isolation. They do not
synthesize Ferric's native `;s` / `;S` key events or drive the native reload,
tab-selector, or saved-session UI workflows. The latter's automatic-reset
contract is supported by the session schema/restore path and fresh-world
runtime evidence, rather than a native restore interaction.

The weak-reference stress check selected the collector-provided pane A record,
ran the production target setter, cleared the Hint tracking map, invoked
Chromium's `HeapProfiler.collectGarbage` twice, and only then ran the production
scroll expression. The connected element remained available through its weak
reference and moved without another strong handle being retained by the test
harness. DevTools was enabled only on the disposable qualification instance;
production remote debugging remains disabled.

The native host run also caught and verified a re-entrancy regression: Rust
originally left Hint mode synchronously, which made QML clear the page-side
hint map before the asynchronous target setter consumed its record. Page-backed
hint actions now keep the session alive until their page callback runs, then
close it. The same run verified the selected pane moved and that selection,
status, and reset confirmations remain visible independently of the normal
status-bar mode.

A final Chromium regression run also used a vertically scrollable `<a>`
element. The `scrollables` collector and fresh validator both retained its
`scrollable` kind (rather than reclassifying it as a link), assignment selected
it, and the generated scroll expression moved that element.

## Repository gates

The following repository gates passed:

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-core -p ferric-browser-ipc -p ferric-browser-engine-qt --locked --offline`
- `cargo test -p ferric-browser --locked --offline`
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
- `cargo xtask docs`
- `cargo xtask check --locked --offline` (including 32 QML tests and workspace tests)
- `cargo build -p ferric-browser --locked`
- `cargo xtask test adapter`
- `git diff --check`

`cargo xtask test wayland` passed its bounded nested native startup smoke. Its
interactive input qualification remains unavailable inside that nested
compositor because it lacks the virtual-keyboard protocol required by `wtype`.
Native `;s`, `;S`, `j`, and status-command input were instead qualified in a
temporary Ferric instance on the host compositor. Native reload, tab selection,
and saved-session restoration were not driven. Top-level navigation and per-tab
policy isolation were exercised at the live page-world level.
