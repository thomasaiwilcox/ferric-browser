# M1-13 caret and document selection evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: the ordinary-DOM caret movement and selection-copy portions of
  PAGE-002, plus the selection subject in ACTION-002/ACTION-003.
- Files: `crates/browser-core/src/action.rs`,
  `crates/browser-core/src/input.rs`, `crates/browser-engine-qt/src/lib.rs`,
  `crates/browser-engine-qt/qml/Main.qml`, `crates/browser-config/src/lib.rs`,
  and `crates/rustbrowser/src/main.rs`.
- Observable result: `v`/`mode-enter caret` enters caret mode; `h/j/k/l`, `w`,
  `b`, `0`, `$`, `caret-move`, and `caret-select` use browser selection APIs
  with character, word, and line granularities. `yank selection`,
  `caret-yank`, `action selection copy`, and the normal-mode `y` binding
  capture the current live document target.
  QML extracts only `window.getSelection()` from the active WebEngineView;
  native code rejects stale targets, password-field results, invalid/NUL data,
  and selections larger than 1 MiB before queuing the clipboard handoff.
  Selection contents remain hidden in the transient re-copy chrome.

## Evidence

- `cargo test -p browser-core -p browser-engine-qt -p rustbrowser --locked
  --offline` passes, including typed selection action and IPC mapping tests.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
  and `cargo build -p rustbrowser --locked --offline` pass.
- Browser-owned one-shot DOM scripts are dispatched through the immutable QML
  resource in `WebEngineScript.ApplicationWorld`; the bundle exposes an
  explicit version marker and never receives page-facing native objects. The
  Qt build also generates `browser-script-manifest.json`, records the SHA-256
  of the packaged `Main.qml` script bundle, and embeds that manifest under the
  same QML resource namespace.
- Selection/editor extraction has a two-second Rust-side deadline, and hint
  collection/target validation discard callbacks that arrive after the
  200-millisecond interaction budget. These deadlines discard late results;
  Qt does not expose safe JavaScript termination, so the page remains
  recoverable through navigation or tab close.
- On the active Wayland backend, the selected-text fixture loaded with a
  healthy renderer. A live `action selection copy` request completed through
  the IPC owner, and `wl-paste --no-newline` returned:
  `Visible selected text for the RustBrowser caret copy smoke.`
- The same smoke reports `qt_surface: available`, `display: wayland`, and no
  owner-side runtime diagnostics.
- On the active Wayland backend, the Unicode caret fixture accepted
  `mode-enter caret`, `caret-select on`, `caret-move right --count 7`, and
  `caret-yank`; `wl-paste --no-newline` returned `Unicode`. The process exited
  without owner-side runtime diagnostics.
- On the active Wayland backend, the focused textarea fixture accepted
  `edit-text` with a direct `/usr/bin/sed` argv configured in
  `docs/testing/fixtures/editor.toml`; the edited value was applied only after
  target/original-value validation and was then returned by `yank selection` as
  `after external edit`. The private runtime editor file was removed after
  completion.

- Editor scratch-directory creation, exclusive 0600 file creation, and initial
  contents write/fsync are acknowledged by a bounded worker before the editor
  process is launched; cancellation or a stale acknowledgement removes the
  file without starting the editor.

## Limitations

The external editor currently relies on a configured direct executable/argv;
cancellation/kill behavior is implemented with a bounded process-group policy.
Failed editor completions now include bounded, sanitized stderr at the existing
editor error boundary; terminal-launcher integration remains. Rich editors and canvas-backed
editors are rejected unless the focused control explicitly advertises the
plain-text-only contenteditable mode, preserving rich formatting. Primary selection, drag/drop, IME, and
broader input-matrix qualification remain covered by WL-003/A11Y-004 rather
than being claimed by this slice.
