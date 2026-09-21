# M2-07 external process and userscript evidence

Date: 2026-09-19  
Status: in-progress

## Task cards

- Requirement: the direct-process portion of SCRIPT-001.
- Files: `crates/ferric-browser-core/src/command.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`,
  `crates/ferric-browser-engine-qt/qml/Main.qml`, and
  `crates/ferric-browser/src/main.rs`.
- Observable result: `spawn -- PROGRAM [ARG...]` accepts a direct argv vector
  through the command, CLI, and typed IPC paths. `{url}` and `{title}` resolve
  as data, `{selection}` requests the current target-aware document selection
  asynchronously, and `{hint_url}` is rejected outside a userscript hint
  invocation. The executable path is never selected from page data and no
  implicit shell is introduced. Child completion is tracked through the
  existing bounded operation status model. Direct spawns and detached action
  targets run with a private browser-owned working directory rather than
  inheriting the browser's ambient current directory; transient profiles use
  a session-scoped directory removed with transient resource release.

The SCRIPT-002/003 slice adds a local manifest registry under the configured
`userscripts/` directory. Manifests are capped at 64 KiB, use schema version 1,
validate namespaced action declarations and declared context fields, resolve
relative executables against the manifest directory, and gate private-profile
execution with `allow_private`. `spawn --userscript NAME` and the spec-facing
`script-run NAME` form are available through the command, CLI, and typed IPC
paths; both use the same manifest validation and executor.
Normal-profile Settings now provides an explicit local-manifest installer. It
refuses private/ephemeral installation, symlinked or non-regular inputs,
existing names, unsafe relative components, oversized assets, and invalid
executables/page sources. Relative executable and page-source assets are copied
under a script-specific directory with private permissions; installation is
create-only, and the generated action registry is immediately queryable.
Manifest installation, inventory reads, and enable-state replacements are
performed by the bounded `ferric-browser-userscript-manager` worker; QML receives
only asynchronous completion through the existing poll handoff.
The same worker owns confirmation-gated removal, staging the manifest before
deleting only its private copied assets and restoring it if cleanup fails.
The explicit `jseval [--world isolated|page] SCRIPT` command is also available
through interactive, CLI, and typed IPC paths. Scripts are capped at 64 KiB,
reject disallowed controls, default to Qt's isolated application world, and
use the page world only when the caller explicitly selects `--world page`.
The active tab ID is captured into a JSON-safe engine action so tabs and
scripts containing whitespace, quotes, or newlines cannot corrupt dispatch.

The protocol sends one versioned JSON document on stdin, closes stdin, exports
only requested `RB_URL`, `RB_TITLE`, `RB_MODE`, and `RB_PROFILE` values, and
accepts bounded line-delimited JSON `message`, `open`, `yank`, and `command`
results. `open` and `yank` results are converted to the existing typed command
path; `command` results require the manifest's command allowlist. The captured
tab/document target is checked again before any result is applied. A manifest
requesting `selection` receives a target-validated, bounded asynchronous page
selection before the process starts. Process completion is reported by the
existing `operations.query` status model. A caller must explicitly request
`operations.query` with `include_stderr: true` to receive retained stderr;
the value is capped at 64 KiB, control-sanitized, and redacts secret-looking
key/value fields and URL query secrets. Default operation responses and
completion events omit it.
`operations.cancel` sets a cancellation token consumed by the worker, kills the
child, and preserves the cancelled state when the worker drains its completion.
Hint activation now permits a manifest that declares `hint_url`: the selected
link is revalidated and its sanitized URL is included in the versioned JSON
context, while ordinary `spawn --userscript` continues to reject that
hint-only field.

Manifest `[[actions]]` entries are now indexed with bounded fields, duplicate
ID detection, built-in collision protection, and stable ordering. Action
discovery includes the generated metadata and private-profile availability;
link and selection context menus expose the available registrations. A hint
target's right-click action menu offers the same available link registrations;
activation re-runs the existing fresh-target probe and passes the captured
subject through the userscript protocol with URL/selection validation and the
normal target recheck. The normal `action SUBJECT VERB` command path also
resolves registered URL, link, selection, tab, window, and context actions;
selection commands use the existing asynchronous target-aware selection
request, while tab, window, and context subjects capture and revalidate their
declared `tab_id`, `window_id`, or `context_name` target, including its stable
generation/profile affinity. Hint-only `hint_url` context is only supplied by
hint activation. Typed action.execute userscript requests accept the
subject-specific value field (url, selection, download_id, history_id,
bookmark_id, quickmark_name, session_name, or command_id); live tab, window,
and context actions may additionally carry their bounded subject identifier
and fail stale instead of falling back to the current object.
Manifest validation now also requires every action to declare the context
field its subject executor needs; link actions may declare hint_url instead
of url only for hint activation.
Such hint-only actions are exposed with source=hint and are rejected by
typed action.execute, preventing IPC/UI callers from discovering an action
that requires a hint capture.

## Evidence

- `cargo test -p ferric-browser-core -p ferric-browser-engine-qt -p ferric-browser --locked
  --offline` passes, including typed argv and CLI forwarding tests.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
  and `cargo build -p ferric-browser --locked --offline` pass.
- On the active Wayland backend, a direct `/usr/bin/touch` spawn created an
  isolated marker file. A second spawn explicitly selected `/usr/bin/sh` and
  wrote the complete selected-text fixture value received through `{selection}`
  to a marker; the value was not split into multiple arguments.
- The owner process reported no stderr diagnostics during the smoke.
- Unit tests cover strict manifest/action validation, result declaration and
  protocol bounds, typed `open`/`yank`/`command` result parsing, plus CLI and
  typed-IPC userscript forwarding.
- The live Wayland smoke installed `fixtures/userscript-message.toml` under an
  isolated configured `userscripts/` directory and confirmed the child was
  started with the versioned stdin context and returned `userscript-ok`.
- A live Wayland sleep-manifest smoke invoked `operations.cancel` over the
  authenticated AF_UNIX connection and observed the operation transition from
  `running` to `cancelled`; the child was stopped before its 30-second sleep
  completed.

## Limitations

Download, history-entry, bookmark, quickmark, session, and command
registered-action subjects
now require the matching typed download ID/history ID/bookmark ID/quickmark
name/session name/command ID, validate the target against the current profile
snapshot or command registry, and expose only the manifest-requested bounded
fields. Live JavaScript result/side-effect
qualification and live Wayland qualification remain follow-up slices. Manifest
allowlists are policy checks;
they do not replace operating-system permissions. The browser does not infer
shell syntax; callers that explicitly choose a shell own that shell's semantics.
