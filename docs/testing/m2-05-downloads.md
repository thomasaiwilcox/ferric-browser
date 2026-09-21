# M2-05 download/PDF evidence

Date: 2026-09-19  
Status: in-progress

## Task card

- Requirements: the filename and lifecycle portions of FILE-002 and FILE-003,
  the explicit open/reveal portion of FILE-004, and the initial PDF portion of
  FILE-005.
- Files: `crates/browser-storage/src/downloads.rs`,
  `crates/browser-storage/src/lib.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: QtWebEngine download requests are explicitly accepted
  only after Rust sanitizes the suggested basename and selects a destination
  under the user Downloads directory. Path separators, controls, empty/dot
  names, Windows device names, excessive lengths, existing files, and final
  symlink paths are handled conservatively. Normal profiles persist offered,
  selecting-destination, in-progress, paused, interrupted, cancelled, and
  completed metadata; private profiles skip the durable index while still
  allowing an explicit user download. The QML downloads manager displays
  lifecycle state, received bytes, known total bytes, a bounded live transfer
  rate estimate, destination, and an engine-provided failure reason when a
  transfer is interrupted.
  The typed `download URL` command and `browser.link.download` action queue a
  target-validated page download through the public QML bridge; rapid link
  hints use the same bounded request path and then recollect hints. The
  `downloads` command opens the existing QML manager from the shared command
  path.
  Completed entries expose explicit Open and Show controls, and the
  `download-open ID`/`download-show ID` command and IPC paths use the same
 Rust-validated filesystem lookup before handing a file URL to the desktop.
  Active entries expose Pause/Resume and Cancel, while interrupted/cancelled
  entries expose Retry. These controls validate lifecycle state in Rust and use
  the live QML download object or a validated source URL for the final engine
  action.
  When `downloads.ask_destination` is enabled, the primary window retains the
  live request in a native Qt `FileDialog` until the user chooses or cancels;
  selected paths are checked for an absolute safe basename and collision.
  Secondary windows now retain normal download requests through the same
  destination picker, collision validation, lifecycle bookkeeping, and
  shutdown cancellation path instead of silently using their default
  directory. Popup downloads use Qt's initiating `WebEngineView` identity to
  route into the popup's own picker, lifecycle state, and shutdown boundary.
  Primary-window engine file-dialog requests now use asynchronous native
  `FileDialog`/`FolderDialog` surfaces for single-file, multiple-file, save,
  and directory-upload modes, preserve request MIME types as desktop filters
  where they can be mapped, and return only local paths. A pending request is
  bound to its WebEngineView and is rejected on navigation, view destruction,
  window teardown, or replacement by another dialog. Duplicate dialogs and
  non-local selections are rejected explicitly.
  The typed `save-page PATH` command validates an absolute, non-colliding
  destination in Rust, captures the active tab, and invokes Qt WebEngine's
  MIME-HTML save operation. Its asynchronous download is routed through the
  same lifecycle/index manager, but retains the explicit destination instead
  of applying the normal download-directory or destination-picker policy.
 The typed `print-pdf PATH` command validates an absolute, non-colliding
 destination in Rust, captures the active tab, and invokes the engine's
 built-in PDF renderer. Completion verifies that the output is a regular
 file and reports renderer failure separately. The `quit` command and primary
  The typed `print` command renders to a private temporary PDF and queues
  direct `lp` submission on a bounded worker. The temporary file and private
  staging directory are removed after the worker reports the printer result;
  missing printer tooling, printer rejection, stale requests, and
  PDF-generation failure are reported as distinct failure outcomes.
  window close path now use the core shutdown transition; if downloads are
  active, the user must choose between keeping the browser open and cancelling
  downloads before exit. Pending print staging files are also removed during
  profile replacement or transient-profile teardown, and the desktop `lp`
  submission process is bounded to 30 seconds with process-group cleanup.
- Evidence: 24 `browser-storage` tests, workspace clippy with `-D warnings`,
  `cargo build -p rustbrowser --locked --offline`, a timed native Wayland launch
  with `--temp-basedir`, a live local-fixture PDF smoke producing a valid
  one-page A4 PDF, and typed save-page/IPC contract tests.

## Limitations

When `downloads.ask_destination = false`, the configured download directory is
used automatically. With the default or enabled setting, the owning window
opens an asynchronous native Qt save chooser and validates the selected path
before accepting the live request. Primary- and secondary-window quit decisions now cancel
active downloads explicitly before the core shutdown transition. Completed
engine downloads are written to a private per-profile staging directory and
copied to the validated final destination with exclusive creation, so the
finalization path handles cross-filesystem destinations and late collisions
without following a destination symlink. Reveal uses the containing
directory URL; desktop file-selection qualification and platform-specific
select-in-folder behavior remain.
Download requests initiated by a page link depend on the engine accepting a
scripted anchor activation; a native gesture-preserving download adapter is
still a follow-up. The remaining shutdown work is unqualified rather than
silently reported as complete. Pause and resume are
wired to QtWebEngine where the request supports them, but sustained
engine/filesystem qualification remains. Primary, secondary, and popup windows
now have the same file/folder picker lifecycle, with each popup keeping its
download request isolated from its opener and other windows. Temporary-PDF cleanup
through failure, replacement, and transient teardown is covered by the Rust
cleanup path; native failure-case qualification remains. Portal print
integration and native printer/UI qualification remain required.
