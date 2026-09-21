# M2-06 IPC transport evidence

Date: 2026-09-19  
Status: in-progress

## Task card

- Requirements: the transport, framing, and initial live method/query subset
  in IPC-001 through IPC-004, plus the retry primitives in IPC-006.
- Files: `crates/browser-ipc/Cargo.toml`, `crates/browser-ipc/src/lib.rs`,
  `crates/rustbrowser/src/main.rs`,
  `crates/browser-engine-qt/src/lib.rs`,
  `crates/browser-engine-qt/qml/Main.qml`, and the workspace manifest/lockfile.
- Observable result: the shared IPC crate provides a local AF_UNIX listener
  rooted in a stable digest-based instance directory, private directory/socket
  modes, Linux peer-UID inspection, create-new instance ownership, dead-PID
  stale-lock reclamation, safe stale socket replacement, bounded big-endian JSON frames, recursive duplicate-key
  rejection, JSON depth validation,
  typed request/response envelopes, and a bounded 60-second per-connection
  response cache that rejects same-ID payload changes. The application now
  acquires the instance lock and starts this listener before creating Qt; the
  listener negotiates `hello`, queues authenticated requests for the Qt thread,
  and returns responses over the original connection. `command.execute`,
  `action.execute`, `actions.query`, `windows.query`, `tabs.query`,
  `profiles.query`, `downloads.query`, `contexts.query`, `switcher.query`, and
  typed stable-ID `switcher.activate` actions,
  `bindings.explain`,
  `site.status`, `diagnostics.get`, and the in-memory operation query/cancel
  methods use typed reducer-backed or sanitized query adapters. `actions.query`
  is backed by stable namespaced action definitions, and `action.execute`
  resolves those IDs to the same typed command path while returning the action
  ID in its structured result. Action rows declare required capabilities and
  report bounded runtime availability reasons; typed and browser-owned UI
  execution recheck that availability immediately before dispatch. URL/link
  Each built-in action row also declares a deterministic completion-provider
  and availability-predicate category from the shared registry; the runtime
  availability response repeats the predicate beside its current state and
  reason. These metadata fields are exposed by `actions.query` and generated
  action documentation. Dynamically discovered configured external-target and
  userscript action rows expose the same completion-provider and
  availability-predicate fields, including the predicate repeated in runtime
  availability, so clients do not need a separate discovery schema for those
  action sources.
  URL/link
  copy and clean-copy actions use the
  typed `yank` executor; native code queues only a safe URL representation for
  QML clipboard delivery, and transient chrome offers the copied value and a
  re-copy action. The documented `action SUBJECT VERB [ARG]` and
  `action-list [SUBJECT]` command forms now resolve through that same registry;
  action-list opens the native library surface and typed IPC command requests
  return structured action metadata. The accept loop
  caps live connections at 32 and each connection's queued/in-flight requests
  at 64; excess work receives `E_BUSY` without unbounded allocation.
  `events.subscribe` returns a state-revision watermark and pushes sanitized
  revision events through a bounded 32-subscriber registry and bounded
  256-message/4 MiB writer queue per subscriber; optional exact
  event-type filters are applied before queueing. Rich event payloads identify
  the affected opaque window/tab/profile/document targets and state changes
  without URLs, titles, search text, or credentials. When backpressure replaces
  queued events, the subscriber receives an `events.gap` marker with a refresh
  watermark; asynchronous userscript/external-process completion publishes an
  `operation.completed` event with only the operation ID and a bounded status
  category. Accepted runtime configuration changes publish a
  `config.changed` event with the new sequence watermark as its revision; a
  single download lifecycle boundary publishes sanitized `download.changed`
  events containing only the opaque download ID, state, and received-byte
  count; action outcomes publish a bounded `action.audit` event containing
  only the action ID, operation ID, outcome, and redacted failure category; a
  persistently slow consumer is then disconnected after two gap
  markers. The backpressure regression test keeps the queue saturated across
  both replacement attempts and verifies that the subscriber is closed on the
  next overflow.

  All IPC URL redaction paths use the same conservative credential-key family:
  OAuth access/refresh tokens, client secrets, credentials, JWT/bearer values,
  signatures, and percent-encoded spellings are removed; malformed encoded
  query names are omitted rather than passed through. Userscript stderr key/value
  redaction uses the same boundary.

  Read-only query adapters reject unknown fields and mistyped booleans,
  strings, and limits at the boundary. `operations.query` omits userscript
  stderr unless `include_stderr: true` is explicitly requested; retained
  stderr is bounded and sanitized. Empty-parameter methods accept only an
  empty object or null, and download snapshots carry the same state sequence
  watermark used by other snapshot responses.

  A second no-argument launch now sends the typed `window.focus` request to the
  existing owner. The owner resolves its last-focused (or active) window,
  queues the same browser-owned activation action used by switcher/window
  focus, and returns a bounded acknowledgement without opening a new tab.
- Evidence: `cargo test -p browser-ipc --locked --offline` (11 tests,
  including persistent backpressure disconnect),
  `cargo test --workspace --locked --offline`, workspace clippy with
  `-D warnings`, and `cargo build -p rustbrowser --locked --offline` all pass.
  A two-process smoke using the same disposable `--basedir` also passes: the
  owner starts with `https://first.example.test`, the second process submits
  `https://second.example.test`, completes the hello handshake, and exits with
  `Forwarded startup input to the running RustBrowser instance`. A timed
  native Wayland launch with `--temp-basedir` remains display-startup
  verified. A live AF_UNIX smoke returns the stable action IDs from
  `actions.query`; `action.execute` for `browser.url.explain` returns the same
  typed preview result with `action_id: browser.url.explain`. A follow-up live
  smoke invokes typed `action-list` for `url` and generic typed `action url
  explain URL`; the former returns three registry rows and the latter returns
  the sanitized preview with its stable action ID.

The CLI client now has separate typed parsing for `open`, `command`,
`query tabs`, `query active-tab`, `query bindings`, `query contexts`,
`query switcher`, `query config`, `diagnostics`, and `config check`. Queries
use the same hello negotiation and AF_UNIX request path as forwarded startup
inputs; command text is parsed with the shared non-shell command parser before
being sent as typed IPC arguments. `--window active` and `--window
last-focused` now select the corresponding core window for commands and
queries. `open --target tab-bg` creates and navigates a background tab while
restoring the previous foreground tab. Forwarded `open --target window` and
`--target private-window` create the existing QML secondary-window surface;
targeted opens at cold startup are rejected rather than silently changing the
requested target.
Live-owner smoke coverage queried tabs, executed an `open` command, and queried
the active tab without launching a second GUI.

The browser-owned UI action adapter now converts compact context-menu and row
values into the same typed argument objects before dispatch. Bookmark,
quickmark, and named-session add/edit/list/load/delete actions, history clear,
tab focus/close/give/suspend/discard/resume, and optional tab state actions
therefore share IPC argument validation and final capability/availability
checks instead of using UI-only parsing. Multi-field UI values are bounded tab
separated fields; history-clear UI input is accepted only as a bounded JSON
object. Current-URL clean/open actions and URL/link/selection/tab send actions
use the same typed translation, including target and optional URL fields. The
adapter regression suite covers valid field mapping and malformed field
rejection.

CLI failures now carry stable symbolic codes and exit statuses for invalid
arguments (2), missing instances/targets (3), permission refusals (4),
unsupported capabilities (5), busy/timeouts (6), and protocol mismatches (7),
with other operation failures using status 1.

Typed `command.execute` and `action.execute` success results also carry a
bounded `command_context` envelope with the IPC source, operation ID, count,
captured window/tab/profile identity, profile privacy, context metadata,
requested routing values, and open target. Valid-command failures include the
same envelope inside error details while continuing to omit command arguments,
URLs, titles, page content, and credentials.

## Limitations

The remaining V1 method surface is now limited to richer binding/config
semantics and full live query/schema qualification; query adapters now reject
unknown or mistyped parameters before accessing state. Event filters are exact event-type
matches, and event payloads identify affected opaque objects with explicit gap
markers under backpressure. Profile
selection is validated against the active Qt profile for existing-window
commands; the secondary-window surface can start a requested durable profile,
but IPC queries are still scoped to the primary BrowserUi; explicit
same-profile `open --context` routing targets the selected existing window.
`switcher.query` validates scope, bounded offset, and limit inputs, tokenizes case-insensitively,
and returns deterministic typed rows across live tabs/windows/contexts,
commands, profile-local history, bookmarks, quickmarks, named sessions, and
downloads. The native surface keeps a bounded in-memory list of eligible
recently closed descriptors and safely reopens a selected closed row through a
new reducer tab. Private rows stay excluded unless both configuration and the
request opt in. Stored-result dispatch now has a Rust allowlist and is wired
through native action buttons; see `docs/testing/m2-13-switcher.md`.
Compositor-level activation remains follow-up work. The initial action registry
currently covers URL open/clean/explain/copy/clean-copy/send, link
open/copy/clean-copy/download/send, selection copy/search/send, tab
send/focus/close/move/mute/pin/undo, and download open/show/cancel/pause/resume/retry; its
command/list adapter is now live. Subject-aware context-menu link/selection
activation includes configured external link and selection targets, while
configured URL and tab targets are also available as bounded switcher action
rows and are revalidated through the shared direct-argv executor before
activation. All configured external target IDs returned by action discovery are
also accepted by typed `action.execute`, with subject-specific URL validation
and the same private-mode checks. Typed window focus, stored-entry open/delete
actions, session load actions, command help/execute actions, and typed download
pause/resume controls are available. Built-in capability metadata currently
covers external targets, live-document selection search, durable
downloads/profile/context storage, compositor window movement, and the command
  registry. Dynamic userscript and external-target rows now carry the same
  capability and predicate metadata; live fixture
execution, subject-bearing switcher activation, and confirmation parity remain
follow-up work. Argument-free registry actions are now switcher rows with
`source=switcher` activation provenance; bookmark, quickmark, and named-session
listing actions plus the confirmation-gated history-clear action are included
alongside the corresponding stored-entry and session mutation actions.
Argument-bearing command actions accept a bounded typed
object and reuse per-command validation; see
`docs/testing/m0-81-action-command-arguments.md`. Hint link-copy and configured external link
targets are available through the hint target surface. Selection search uses
the configured HTTPS engine templates and the browser-owned one-shot selection
boundary; see
`docs/testing/m0-73-action-selection-search.md`.
`config.get` exposes validated global and site-scoped keys through the setting
registry and reports matching site-rule contributors when requested;
`bindings.query` reports the effective built-in trie plus user overrides from
the authored, profile, persistent, CLI, and temporary layers. Each configured
keychain includes bounded provenance, including explicit unbound masks;
`bindings.explain` exposes the same provenance for its winning binding.

The `hello` capability list now advertises every implemented read-only query,
binding explanation, permission/status query, and switcher/window activation
method so clients can discover the available surface before issuing requests.
`contexts.query`
reads the durable UUID/profile-affine registry; `context-list`, `context-create`
(including `--workspace`), and confirmation-gated `context-delete` are
available through the shared command and CLI paths. `context-enter` assigns the
current normal window after checking profile affinity, and `context-save`
atomically captures its live window/tab membership. Context routing and
declarative `contexts.toml` loading/merge are now supported for a sibling file
of `--config`; cross-profile/new-window context routing carries the validated
target profile and context into the preflighted secondary window. Native
multi-window qualification remains.
Targeted opens at cold startup are still rejected. Startup inputs from a second
CLI process are forwarded when an owner exists, and a no-argument launch now
requests focus through the owner. The transport deliberately has no TCP
listener or browser-page bridge.
