# M1-06 implementation evidence

Date: 2026-09-25
Status: in-progress

## Task card

- Requirements: HINT-001 through HINT-005.
- Files: `crates/ferric-browser-core/src/hints.rs`, the typed configuration and
  command/IPC models, `crates/ferric-browser-engine-qt/src/`, QML runtime
  components, `qml/scripts/BrowserScripts.js`, and the live hint fixtures.
- Observable result: `f` starts the default `hint all` session; `;a` starts the
  action chooser; `gi` focuses the first input and a count such as `3gi`
  forwards `--index 3`. Candidate families cover links, inputs, buttons,
  images, media, and scrollable elements. The isolated-world collector walks
  same-origin frames and open shadow roots, clips geometry, performs composed
  hit testing, assigns stable document-local element IDs, and reports a dirty
  revision for coalesced live refresh.
- Rust validates the bounded snapshot, assigns configurable prefix-free labels,
  preserves them across same-width refreshes, owns prefix/text-filter state and
  ranking, and revalidates the exact tab/document/frame/element before every
  activation. QML presents active/unmatched states, collision-aware placement,
  target outlines, status counts, mouse parity, and the applicable-action
  palette.

## Explicit scroll target follow-up

`scroll-target select` enters the existing Hint mode with the `scrollables`
family. Selection stores only weak references to the exact element and its
owner document in the isolated application world; closing the Hint session
releases the collector's strong element map. `scroll-target document`,
`scroll-target auto`, and `scroll-target status` expose the three distinct
policies. Scroll ownership remains independent of DOM focus, and each operation
checks frame reachability, composed visibility, and axis-specific scrollability
again. The live fixture and per-case qualification evidence are maintained in
[`explicit-scroll-target.md`](explicit-scroll-target.md).

The native Wayland startup smoke passed during scroll-target qualification.
The interactive matrix was not run through Ferric's native key path because
the compositor lacks the virtual-keyboard protocol required by `wtype`; the
live Chromium page-script matrix is separate evidence and is not presented as
a native input pass.

- Rapid actions remove consumed stable IDs while preserving the remaining
  labels and retain the 20-background-tab confirmation boundary. Navigation,
  stale identity, replacement, covering, or material movement fails closed.
- Verification target: locked workspace tests, Clippy with warnings denied,
  formatting, build, QML adapter tests, and the native Wayland smoke where a
  compositor is available. Performance remains p95 no more than 200 ms for 500
  visible candidates at three zoom levels.

## Limitations

The collector runs in Qt WebEngine's isolated application world through the
public QML `runJavaScript` API. It represents the top document as frame `0` and
walks same-origin descendants to depth eight, carrying frame paths and
transforming geometry into top-view coordinates. Collection remains capped at
5,000 candidates. Browser features requiring a trusted user-activation event,
cross-origin frames, closed shadow roots, canvas internals, PDF UI, and
engine-specific device-scale/zoom qualification remain explicit exclusions.

## Rapid hint follow-up

The command, typed IPC, and CLI paths accept `--rapid` with the bounded targets
`current`, `tab-bg`, `yank`, `clean-yank`, `download`, and `userscript`. Rapid
selection copies, downloads, opens, or dispatches userscript context only after
fresh validation, acknowledges and removes the consumed candidate, then diffs
the dirty snapshot without blanking the overlay. Reaching 20 background tabs
pauses for explicit confirmation. Unsupported rapid targets fail before any
navigation, clipboard, download, or tab mutation.

## YouTube viewport overflow regression (2026-10-11)

The production collector was evaluated in a disposable Chromium isolated world
on `https://www.youtube.com/` at a 1400 × 1000 viewport. It returned zero
candidates for `all`, `links`, and `buttons` despite 35 anchors with URLs and
50 buttons in the document. YouTube's absolutely positioned application left
the body and HTML layout boxes at zero height; the body had computed
`overflow-x: auto; overflow-y: scroll`, while the HTML root had visible overflow.
The collector incorrectly intersected every target with that empty body box.

[CSS overflow viewport propagation](https://www.w3.org/TR/css-overflow-3/#overflow-propagation)
applies those body overflow values to the viewport and leaves the body's used
overflow visible. Collection, fresh activation validation, and explicit scroll
target visibility now share the same rectangle clipping helper. It distinguishes
viewport overflow from ordinary ancestor clipping, retains body clipping when
the HTML root owns overflow or containment disables propagation, and evaluates
each ancestor in its own document, including same-origin frames.

The deterministic regression is
`crates/ferric-browser-engine-qt/tests/qml/tst_hint_geometry.qml`, included in
`cargo xtask test engine` and `cargo xtask check`. The body and root propagation
cases failed with the old script and passed with the fix in Qt WebEngine's
application world. The tests also cover partial/full clipping, offscreen links,
body containment, fresh rejection after hiding, button activation, explicit
scroll target selection, and nested-frame visibility. All 40 QML tests and all
392 Rust adapter tests passed. The new QML tests also passed with the project's
default runner environment, without extra Chromium flags. QML lint, Rust
formatting, and the whitespace diff check passed.

On the live YouTube consent dialog, the corrected collector returned eight
targets (five links and three buttons); all eight also passed fresh validation.
After rejecting consent in that disposable profile and waiting for the page to
render, it returned 49 `all` targets, 27 `links` targets, and four `buttons`
targets. Every collected target passed fresh validation in its corresponding
session. Live page-script checks use a disposable Chromium profile and do not
claim qualification of Ferric's native keyboard or overlay path.
The rebuilt and installed release was subsequently verified interactively on
YouTube, confirming the reported hint failure is resolved.
