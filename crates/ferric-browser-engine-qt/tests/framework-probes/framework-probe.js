    import {EditorState as CMState} from "@codemirror/state";
    import {EditorView as CMView} from "@codemirror/view";
    import {history as cmHistory, undo as cmUndo} from "@codemirror/commands";
    import {Schema as PMSchema} from "prosemirror-model";
    import {EditorState as PMState, TextSelection as PMTextSelection} from "prosemirror-state";
    import {EditorView as PMView} from "prosemirror-view";
    import {history as pmHistory, undo as pmUndo} from "prosemirror-history";
    import {Editor as TiptapEditor} from "@tiptap/core";
    import StarterKit from "@tiptap/starter-kit";

    const versions = {
      ace: "ace-builds 1.44.0",
      codemirror: "@codemirror/state 6.7.6 + @codemirror/view 6.43.14 + @codemirror/commands 6.11.1",
      monaco: "monaco-editor 0.57.0",
      prosemirror: "prosemirror-model 1.25.12 + state 1.4.4 + view 1.42.6 + history 1.5.1",
      tiptap: "@tiptap/core + @tiptap/starter-kit 3.31.4"
    };
    const expectedText = "alpha beta";
    const changedText = "gamma beta";
    const seedHtml = "<p>alpha <strong>beta</strong></p>";
    const results = [];

    function recordError(name, error) {
      results.push({
        name,
        version: versions[name],
        error: error && error.name ? error.name : "Error",
        detail: String(error && error.message ? error.message : "").slice(0, 160),
        stack: String(error && error.stack ? error.stack : "").split("\n").slice(0, 6).join(" | ")
      });
    }

    function guardedApply(adapter, snapshot, consent) {
      if (!consent) return false;
      if (snapshot.owner !== adapter.owner() ||
          snapshot.document !== adapter.document() ||
          snapshot.revision !== adapter.revision() ||
          (snapshot.nativeRevision !== null &&
           snapshot.nativeRevision !== adapter.nativeRevision())) return false;
      adapter.apply();
      return true;
    }

    function guardedExtract(adapter, consent) {
      if (!consent) return null;
      return adapter.text();
    }

    function capture(adapter) {
      return {
        owner: adapter.owner(),
        document: adapter.document(),
        revision: adapter.revision(),
        nativeRevision: adapter.nativeRevision ? adapter.nativeRevision() : null,
        selection: adapter.selection()
      };
    }

    async function runCases(name, adapter) {
      const rawText = adapter.text;
      let extractionReads = 0;
      adapter.text = () => {
        extractionReads += 1;
        return rawText();
      };
      adapter.reset();
      const focusResult = await adapter.focus();
      const focusApi = typeof focusResult === "object" ? focusResult.api : focusResult;
      const focusReached = typeof focusResult === "object" ? focusResult.dom : focusResult;
      const focusNoExtract = extractionReads === 0;
      const deniedExtract = guardedExtract(adapter, false);
      const extractDenied = deniedExtract === null && extractionReads === 0;
      const extract = guardedExtract(adapter, true) === expectedText;
      adapter.reset();
      const permissionSnapshot = capture(adapter);
      const beforeDenied = adapter.text();
      const permissionNoOp = !guardedApply(adapter, permissionSnapshot, false) && adapter.text() === beforeDenied;

      adapter.reset();
      const staleSnapshot = capture(adapter);
      adapter.externalEdit();
      const externalOnly = adapter.text();
      const staleRejected = !guardedApply(adapter, staleSnapshot, true) &&
                            adapter.text() === externalOnly && externalOnly === "alpha beta!";

      adapter.reset();
      const replacementSnapshot = capture(adapter);
      const oldOwner = adapter.owner();
      const oldDocument = adapter.document();
      const oldRevision = adapter.revision();
      if (document.activeElement && document.activeElement.blur) document.activeElement.blur();
      adapter.replaceInstance();
      const ownerChanged = oldOwner !== adapter.owner();
      const documentChanged = oldDocument !== adapter.document();
      const revisionStable = oldRevision === adapter.revision();
      const ownerReplacementRejected = !guardedApply(adapter, replacementSnapshot, true) &&
                                       adapter.text() === expectedText;

      adapter.reset();
      adapter.selectEnd();
      const applySnapshot = capture(adapter);
      const apply = guardedApply(adapter, applySnapshot, true);
      const afterApplyText = adapter.text();
      const selectionPreserved = adapter.selection() === applySnapshot.selection;
      const formatPreserved = adapter.formatPreserved();
      const undoAvailable = adapter.undo();
      const undoFormat = adapter.formatPreserved();
      const undoRestored = adapter.text() === expectedText &&
                           (undoFormat === true || undoFormat === "not_applicable");
      results.push({
        name,
        version: versions[name],
        focusApi,
        focusReached,
        focusNoExtract,
        extractDenied,
        extract,
        permissionNoOp,
        staleRejected,
        ownerChanged,
        documentChanged,
        revisionStable,
        ownerReplacementRejected,
        apply,
        changed: afterApplyText === changedText,
        selectionPreserved,
        formatPreserved,
        undoAvailable,
        undoRestored
      });
    }

    function makeAce() {
      const host = document.getElementById("ace");
      window.ace.config.set("basePath", "/vendor");
      const Range = window.ace.require("ace/range").Range;
      let editor;
      let session;
      let revision = 0;
      const create = function() {
        if (editor) editor.destroy();
        host.innerHTML = "";
        editor = window.ace.edit(host);
        session = editor.getSession();
        session.on("change", function() { revision += 1; });
      };
      const reset = function() {
        editor.setValue(expectedText, -1);
        session.getUndoManager().reset();
        revision = 0;
      };
      create();
      reset();
      return {
        owner: () => editor,
        document: () => session,
        revision: () => revision,
        text: () => editor.getValue(),
        selection: () => {
          const p = editor.getCursorPosition();
          return p.row + ":" + p.column;
        },
        focus: () => {
          editor.focus();
          return document.activeElement === editor.textInput.getElement();
        },
        reset,
        replaceInstance: () => { create(); reset(); },
        selectEnd: () => editor.moveCursorTo(0, 10),
        externalEdit: () => session.insert({row: 0, column: 10}, "!"),
        apply: () => session.replace(new Range(0, 0, 0, 5), "gamma"),
        undo: () => { editor.undo(); return editor.getValue() === expectedText; },
        formatPreserved: () => "not_applicable"
      };
    }

    function makeCodeMirror() {
      const host = document.getElementById("codemirror");
      let view;
      let revision = 0;
      const makeState = () => CMState.create({doc: expectedText, extensions: [
        cmHistory(), CMView.updateListener.of(update => {
          if (update.docChanged) revision += 1;
        })
      ]});
      const reset = function() {
        const state = makeState();
        if (view) view.setState(state);
        else view = new CMView({state, parent: host});
        revision = 0;
      };
      const replaceInstance = function() {
        if (view) view.destroy();
        host.innerHTML = "";
        view = new CMView({state: makeState(), parent: host});
        revision = 0;
      };
      reset();
      return {
        owner: () => view,
        document: () => view.state,
        revision: () => revision,
        text: () => view.state.doc.toString(),
        selection: () => String(view.state.selection.main.head),
        focus: () => { view.focus(); return host.contains(document.activeElement); },
        reset,
        replaceInstance,
        selectEnd: () => view.dispatch({selection: {anchor: expectedText.length}}),
        externalEdit: () => view.dispatch({changes: {from: 10, insert: "!"}}),
        apply: () => view.dispatch({changes: {from: 0, to: 5, insert: "gamma"}}),
        undo: () => cmUndo(view),
        formatPreserved: () => "not_applicable"
      };
    }

    async function makeMonaco() {
      const monaco = await window.__s22MonacoReady;
      const host = document.getElementById("monaco");
      let editor;
      let model;
      let revision = 0;
      const create = function() {
        if (editor) editor.dispose();
        host.innerHTML = "";
        editor = monaco.editor.create(host, {
          value: expectedText,
          language: "plaintext",
          automaticLayout: true,
          minimap: {enabled: false},
          scrollBeyondLastLine: false
        });
        model = editor.getModel();
        model.onDidChangeContent(() => revision += 1);
        revision = 0;
      };
      const reset = function() {
        model.setValue(expectedText);
        editor.pushUndoStop();
        editor.setPosition({lineNumber: 1, column: 11});
        revision = 0;
      };
      create();
      reset();
      return {
        owner: () => editor,
        document: () => model,
        revision: () => revision,
        nativeRevision: () => model.getVersionId(),
        text: () => model.getValue(),
        selection: () => {
          const p = editor.getPosition();
          return p.lineNumber + ":" + p.column;
        },
        focus: () => { editor.focus(); return document.getElementById("monaco").contains(document.activeElement); },
        reset,
        replaceInstance: () => create(),
        selectEnd: () => editor.setPosition({lineNumber: 1, column: 11}),
        externalEdit: () => editor.executeEdits("s22-external", [{
          range: new monaco.Range(1, 11, 1, 11), text: "!"
        }]),
        apply: () => {
          editor.pushUndoStop();
          const ok = editor.executeEdits("s22-probe", [{
            range: new monaco.Range(1, 1, 1, 6), text: "gamma", forceMoveMarkers: true
          }]);
          editor.pushUndoStop();
          return ok;
        },
        undo: () => {
          editor.trigger("s22", "undo", null);
          return model.getValue() === expectedText;
        },
        formatPreserved: () => "not_applicable"
      };
    }

    function makeProseMirror() {
      const schema = new PMSchema({
        nodes: {
          doc: {content: "block+"},
          paragraph: {
            content: "inline*",
            group: "block",
            parseDOM: [{tag: "p"}],
            toDOM: () => ["p", 0]
          },
          text: {group: "inline"}
        },
        marks: {
          strong: {
            parseDOM: [{tag: "strong"}],
            toDOM: () => ["strong", 0]
          }
        }
      });
      const strong = schema.mark("strong");
      const host = document.getElementById("prosemirror");
      let view;
      let revision = 0;
      const makeState = () => PMState.create({
        schema,
        doc: schema.node("doc", null, [schema.node("paragraph", null, [
          schema.text("alpha "), schema.text("beta", [strong])
        ])]),
        plugins: [pmHistory()]
      });
      const reset = function() {
        const state = makeState();
        if (view) view.updateState(state);
        else view = createView(state);
        revision = 0;
      };
      const createView = function(state) {
        return new PMView(host, {
          state,
          dispatchTransaction(transaction) {
            const next = view.state.apply(transaction);
            view.updateState(next);
            if (transaction.docChanged) revision += 1;
          }
        });
      };
      const replaceInstance = function() {
        if (view) view.destroy();
        host.innerHTML = "";
        view = createView(makeState());
        revision = 0;
      };
      const formatPreserved = () => {
        const para = view.state.doc.firstChild;
        return para.childCount === 2 && para.child(1).marks.some(mark => mark.type.name === "strong");
      };
      reset();
      return {
        owner: () => view,
        document: () => view.state.doc,
        revision: () => revision,
        text: () => view.state.doc.textContent,
        selection: () => String(view.state.selection.head),
        focus: () => { view.focus(); return host.contains(document.activeElement); },
        reset,
        replaceInstance,
        selectEnd: () => view.dispatch(
          view.state.tr.setSelection(PMTextSelection.atEnd(view.state.doc))
        ),
        externalEdit: () => view.dispatch(view.state.tr.insertText("!", view.state.doc.content.size - 1)),
        apply: () => view.dispatch(view.state.tr.insertText("gamma", 1, 6)),
        undo: () => pmUndo(view.state, transaction => view.dispatch(transaction)),
        formatPreserved
      };
    }

    function makeTiptap() {
      const host = document.getElementById("tiptap");
      let editor;
      let revision = 0;
      const reset = function() {
        if (editor) editor.destroy();
        revision = 0;
        editor = new TiptapEditor({
          element: host,
          extensions: [StarterKit],
          content: seedHtml,
          autofocus: false,
          onTransaction({transaction}) { if (transaction.docChanged) revision += 1; }
        });
        revision = 0;
      };
      const formatPreserved = () => {
        const json = editor.getJSON();
        const nodes = json.content && json.content[0] && json.content[0].content;
        return !!nodes && nodes.length === 2 &&
               Array.isArray(nodes[1].marks) && nodes[1].marks.some(mark => mark.type === "bold");
      };
      reset();
      return {
        owner: () => editor,
        document: () => editor.state,
        revision: () => revision,
        text: () => editor.getText(),
        selection: () => String(editor.state.selection.head),
        focus: async () => {
          const accepted = editor.commands.focus();
          await new Promise(resolve => setTimeout(resolve, 250));
          return {
            api: accepted,
            dom: host.contains(document.activeElement) && editor.view.hasFocus()
          };
        },
        reset,
        replaceInstance: reset,
        selectEnd: () => editor.commands.setTextSelection(editor.state.doc.content.size - 1),
        externalEdit: () => editor.commands.insertContentAt(editor.state.doc.content.size - 1, "!"),
        apply: () => editor.commands.insertContentAt(
          {from: 1, to: 6}, "gamma", {updateSelection: false}
        ),
        undo: () => editor.commands.undo(),
        formatPreserved
      };
    }

    async function main() {
      try {
        await window.__s22MonacoReady;
          for (const [name, make] of [
            ["ace", makeAce],
            ["codemirror", makeCodeMirror],
            ["monaco", makeMonaco],
            ["prosemirror", makeProseMirror],
            ["tiptap", makeTiptap]
          ]) {
            try {
              await runCases(name, await make());
            } catch (error) {
              recordError(name, error);
            }
          }
        window.__s22 = {
          status: "ready", versions, results,
          unavailable: [{name: "quill", advisory: "GHSA-v3m3-f69x-jf25",
                         reason: "No patched release; historical results are retained in S22 evidence."}]
        };
      } catch (error) {
        recordError("fixture", error);
        window.__s22 = {status: "failed", results};
      }
      document.body.dataset.ready = window.__s22.status;
    }
    main();
