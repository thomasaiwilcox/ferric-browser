# M1-06 implementation evidence

Date: 2026-09-25
Status: in-progress

## Task card

- Requirements: HINT-001 through HINT-005.
- Files: `crates/ferric-browser-core/src/hints.rs`, the typed configuration and
  command/IPC models, `crates/ferric-browser-engine-qt/src/`, QML runtime
  components, `qml/scripts/BrowserScripts.js`, and the live hint fixture.
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
