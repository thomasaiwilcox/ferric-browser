# M2-11 JavaScript userscript subset evidence

Date: 2026-09-18  
Status: in-progress

## Task card

- Requirement: PAGE-004.
- Files: `crates/browser-engine-qt/src/userscript.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: a local manifest can opt into page-world execution with
  bounded `matches`/`excludes`, `run_at`, `frames`, `source`, and
  `allow_private` fields. Rust loads only matching local sources and QML
  injects them into the page's main world for document-start, document-end, or
  document-idle phases. External executable userscripts remain a separate
  protocol.

## Security boundary

Page-world execution is disabled unless `page_world = true` is explicit. The
source path is resolved relative to the manifest directory, capped at 256 KiB,
and rejected when it is missing, non-regular, contains NUL data, or uses a
relative traversal component. Relative executable paths use the same bounded
path policy; an explicit `./` prefix is accepted. Private profiles skip scripts
unless `allow_private = true`. No native object, file
handle, shell method, or IPC capability is exposed to the page script.

## Evidence

- Unit tests cover match-pattern bounds, exclusions, explicit source/scope
  requirements, local-source loading, private-profile filtering, path
  traversal rejection, and duplicate context/permission declarations.
- Strict Clippy and the Rust/Qt build pass.
- Settings exposes a bounded, path-free installed-userscript inventory with
  per-manifest enable/disable controls. The enabled flag is persisted by
  private-file, sync-before-atomic-replace writes and takes effect on the next
  navigation; disabled manifests are excluded from both page injection and
  action registration.
- Settings also provides a confirmation-gated remove action. Removal is
  worker-owned, refuses symlinked manifests/assets, stages the manifest out of
  the live namespace, removes only its private asset directory, and refreshes
  the inventory after completion.
- Inventory reads and enabled-state replacements run on the bounded
  `rustbrowser-userscript-manager` worker; GUI properties are updated only from
  the existing poll handoff. The worker contract is covered by a dedicated
  read/toggle regression test.
- Removal regression tests cover successful manifest/asset deletion and refusal
  of symlinked private asset directories; inspection failures are now surfaced
  instead of being treated as an absent asset directory.
- The local HTTP fixtures `fixtures/userscript-page.toml`,
  `fixtures/userscript-page-all.toml`, `fixtures/page.js`,
  `fixtures/page-all.js`, and `fixtures/page-frame.html` exercise the
  configured source paths through the QML injection path; the live Wayland
  smoke observed both same-origin marker requests
  `GET /__rustbrowser_userscript_hit__` and
  `GET /__rustbrowser_userscript_subframe_hit__`.
- The same smoke completed the owner IPC query and coordinated quit with no
  QML/runtime diagnostics.

## Limitations

The top-frame path still runs directly through `WebEngineView.runJavaScript`.
For manifests declaring `frames = "all"`, the view's public persistent
`WebEngineScript` collection is replaced at navigation start and one bounded
reload qualifies document lifecycle timing before subframes execute. `@require`,
metadata APIs, and the wider Greasemonkey compatibility surface remain out of
scope.
