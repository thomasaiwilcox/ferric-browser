# S22 framework editor capability report

**Decision:** no-go for generic cross-framework document writeback. The public
APIs support different framework-specific operations, and the fixture shows
that consent and exact target/revision checks can guard a bounded edit in the
pinned local builds. This does not authorize production writeback or establish
live-site support. A future editor-specific adapter remains a separate
research and qualification decision.

## Pinned packages and public API evidence

The exact npm pins are in
[`package-lock.json`](../../../../crates/ferric-browser-engine-qt/tests/framework-probes/package-lock.json).
The test runner installs them into a disposable directory, creates a temporary
browser bundle, and serves the fixture from localhost. No package source or
generated bundle is shipped with Ferric. Direct package licenses are recorded
for provenance; transitive package integrity is retained in the npm lock.

| Framework and pinned release | Focus / extract | Apply and commands | Revision and history boundary | Prototype result |
| --- | --- | --- | --- | --- |
| Ace `ace-builds 1.44.0` (BSD-3-Clause) | `Editor.focus()`, `getValue()` | `EditSession.replace(range, text)`, `Editor.undo()` | The probe listens to the exact `EditSession` change event and checks the exact `Editor` and session objects. | Focus and text extraction worked. Permission denial, stale edit, instance replacement, bounded replace, selection, and undo passed. Rich formatting is not applicable to Ace's text document. |
| CodeMirror 6: `@codemirror/state 6.7.6`, `@codemirror/view 6.43.14`, `@codemirror/commands 6.11.1` (MIT) | `EditorView.focus()`, immutable `state.doc` | `EditorView.dispatch()` transaction; history extension `undo()` | The `EditorState` object is immutable and replaced on updates. The probe also tracks document-changing updates and the exact `EditorView`. | All exercised checks passed. The core document is text; syntax/style extensions are not rich inline document marks. |
| Monaco `monaco-editor 0.57.0` (MIT) | `IStandaloneCodeEditor.focus()`, `getValue()` | `executeEdits()` with undo stops; editor undo action | Exact editor and `ITextModel` identity plus public `getVersionId()`. Monaco increments the model version on edits, including undo/redo. | All exercised checks passed. Formatting is outside the plain text model and is marked not applicable. |
| ProseMirror: `prosemirror-model 1.25.12`, `state 1.4.4`, `view 1.42.6`, `history 1.5.1` (MIT) | `EditorView.focus()`, schema-backed `state.doc` | A `Transaction` through `EditorView.dispatch()`; history plugin undo | Exact view and document references plus a probe-owned revision counter. `EditorState` is persistent; app integration owns transaction dispatch and versioning. | All exercised checks passed, including preservation of an unrelated strong mark and undo. The tested schema has only paragraph, text, and strong-mark nodes. |
| Tiptap `@tiptap/core 3.31.4` and `@tiptap/starter-kit 3.31.4` (MIT) | `commands.focus()`, `getJSON()` / `getText()` | `insertContentAt()` and `commands.undo()` | Exact `Editor` and ProseMirror state identity plus a probe-owned document-change revision. Schema and extensions are application-specific. | Extraction, guarded apply, format, selection, conflict, and undo passed. The focus command returned success but the offscreen DOM-focus check remained false; native focus is unqualified. |
| Quill `quill 2.0.3` (BSD-3-Clause) | `focus()`, `getContents()` / `getText()` | `updateContents(Delta, "api")`, History module undo | Exact Quill instance and root identity plus an app-owned `text-change` revision. Delta is the rich document model; history configuration affects whether API changes can be undone. | All exercised checks passed, including preservation of an unrelated bold Delta run and undo. |

The fixture bundle uses esbuild `0.28.2` (MIT). Its final run recorded npm
lock SHA-256 `08124d7abbcd0ba1b816a286500b9d3b592bd5b7bc2eee56526060eeca2c4f21`.
The full test result and generated bundle hash are in
[`logs/S22/webengine-fixture.log`](logs/S22/webengine-fixture.log).

## Capability decisions

| Capability | Research result |
| --- | --- |
| Focus | Public APIs exist in all six packages. Five reached the expected editor DOM in this offscreen fixture. Tiptap's command reported success while its editor did not become the active DOM element, so that result is unqualified. Focus does not imply permission to extract or apply content. |
| Extract | Public text or structured-document APIs exist in each package. The fixture separately denied reads and confirmed no text was returned; granting extraction read the known local fixture content. It did not persist or log content. |
| Apply | Each package exposes a bounded framework-native editing route. The probe applied one replacement only after consent and exact target/revision validation. Every pinned build restored the original state through its own undo mechanism. This is a local API result, not application integration evidence. |
| Commands | Command APIs are package- and extension-specific. They remain separate from focus and content extraction. No command is inferred from a framework name or document text. |
| Generic writeback | **No-go.** There is no shared public transaction, revision, schema, collaboration, selection, formatting, or undo contract. A generic `.value`, `innerText`, or `innerHTML` write would bypass those differences. |

The local transaction prototype checks all six packages. Permission denial did
not call the apply function. An edit after capture caused a no-op while
retaining the external edit. Replacing the editor and document objects at the
same harness revision caused a no-op. The accepted bounded edit preserved the
end selection; the rich-text models retained the unrelated mark; each
framework undo restored the seed document. The harness's consent and revision
guard are test-only code and are not Ferric permissions or adapters.

## Evidence limits

T050 passed in Qt 6.11.2 / QtWebEngine 6.11.2 / Chromium 140 using this local
fixture. The run emitted two non-fatal `addRange(): The given range isn't in document` WebEngine warnings during the focus/probe lifecycle; the QTest assertions still passed. Tiptap's DOM focus remained false in the same run. These results require native retesting before any focus capability is claimed for Tiptap.

The generic native Qt editor mode/IME suite passed, but it did not mount these
framework package builds. Physical Wayland keyboard input was unavailable
because the nested Weston path lacks the virtual-keyboard protocol needed by
`wtype`. No package-specific native input, IME, collaboration, live-site, or
Google Docs writeback claim is made.

## Official API references

- [Ace Editor API](https://ajaxorg.github.io/ace-api-docs/classes/src_editor.Editor.html) and [EditSession API](https://ajaxorg.github.io/ace-api-docs/classes/src_edit_session.EditSession.html)
- [CodeMirror guide](https://codemirror.net/docs/guide/) and [reference](https://codemirror.net/docs/ref/)
- [Monaco editor API](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IStandaloneCodeEditor.html) and [text model API](https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.ITextModel.html)
- [ProseMirror guide](https://prosemirror.net/docs/guide/) and [reference](https://prosemirror.net/docs/ref/)
- [Tiptap editor API](https://tiptap.dev/docs/editor/api/editor), [commands](https://tiptap.dev/docs/editor/api/commands), [setContent](https://tiptap.dev/docs/editor/api/commands/content/set-content), and [undo/redo](https://tiptap.dev/docs/editor/extensions/functionality/undo-redo)
- [Quill API](https://quilljs.com/docs/api) and [History module](https://quilljs.com/docs/modules/history)
