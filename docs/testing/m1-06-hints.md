# M1-06 implementation evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: HINT-001, HINT-002, HINT-003, the link/input portion of
  HINT-004, and the yank portion of HINT-005.
- Files: `crates/ferric-browser-core/src/hints.rs`, `crates/ferric-browser-core/src/id.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, and `docs/requirements.csv`.
- Observable result: `f` maps to qutebrowser's baseline `hint all` behavior.
  The Qt adapter asks the current WebEngineView for a bounded
  visible-interactive-element snapshot, including links, images, summaries,
  buttons, form controls, explicit click handlers, tabbable and ARIA controls,
  and open shadow-root descendants. Rust
  validates the untrusted JSON, caps candidates at 5,000, assigns deterministic
  `asdfghjkl` labels in viewport order, and binds them to a hint session and
  captured tab/document target. QML renders compact native label overlays.
  Selection resolves the exact retained element identity and rechecks target,
  frame, attachment, visibility, geometry, material movement, and metadata,
  then performs direct validated navigation for links
  through `browser.link.open`, focuses form/contenteditable controls and
  enters insert mode, or invokes bounded DOM activation for validated ordinary
  controls.
  Rapid `hint --rapid --target yank` link activation copies the validated,
  privacy-filtered URL and recollects labels while the source document remains
  in hint mode.
  `hint --target userscript --script NAME` validates the selected link, passes
  its sanitized URL as the manifest-declared `hint_url` context, and starts
  the bounded userscript operation.
- Evidence: `cargo test -p ferric-browser-core --locked` (42 tests),
  `cargo test -p ferric-browser-engine-qt --locked` (7 tests), workspace clippy with
  warnings denied, and a live offscreen smoke using
  `docs/testing/fixtures/hints.html`: IPC `command.execute` with `hint` was
  accepted and `windows.query` reported `mode: hint`, proving the DOM collector
  retained a non-empty label set.

## Limitations

The collector runs in Qt WebEngine's isolated application world through the
public QML `runJavaScript` API. It represents the top document as frame `0` and walks
same-origin `iframe`/`frame` descendants to a bounded depth of eight, carrying
frame paths and transforming descendant geometry into top-view coordinates.
Collection is bounded by candidate count and frame depth, and excludes targets
outside both their owning frame viewport and the top-level viewport. Completed
renderer results are not discarded based on wall-clock latency. Navigation
invalidates the captured browser document; fresh selection uses the exact
isolated-world element retained for the session, recomputes its same-origin
frame-chain geometry, and checks attachment, viewport visibility, metadata,
and material movement. This avoids false rejection from unrelated mutations on dynamic
pages and prevents an overlapping child, image, or shadow host from retargeting
activation.
Browser features which require a trusted user-activation event,
cross-origin/closed frames, and engine-specific device-scale/zoom qualification
remain. Rapid yank and userscript actions are limited to validated links and
do not create tabs; rapid download uses the same validated link target but
depends on scripted anchor activation. Canvas controls, closed shadow roots,
PDF UI, and inaccessible frames remain explicit exclusions per the
specification.

## Rapid hint follow-up

The command, typed IPC, and CLI paths accept `--rapid` with the bounded
targets `current`, `tab-bg`, `yank`, `clean-yank`, `download`, and `userscript`.
Rapid yank, clean-yank, background-tab, download, and manifest-backed userscript selection either copy
the safe URL, queue a validated download, open it in a new background tab
through the normal target-aware navigation path, or start the userscript with
sanitized `hint_url` context. These actions leave
the mode stack in hint mode, clear the stale session, and ask QML to recollect
the current document. Background-tab creation is capped at 20 per batch;
reaching the boundary pauses the session for an explicit QML confirmation that
grants one more bounded batch.
Unsupported rapid targets return an explicit error without a navigation or tab
mutation. Unit coverage exercises typed and CLI option forwarding; workspace
Clippy and `cargo xtask check` pass.
