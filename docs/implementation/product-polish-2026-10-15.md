# Product polish pass — 2026-10-15

Status: active. Branch: `polish/refined-browser-experience`.

Latest verified checkpoint: new-window creation and profile/context inheritance
across primary, secondary and popup owners, including private/temporary identity
and routing decisions in the requesting window. The local gate, broad native
integration, clipboard, live IPC and adapter checks pass. Production captures
confirm dialog placement and readable controls; routing-card spacing remains a
visual follow-up. The full polish goal remains active, including shared child
hints, continuing permission/journey state and the remaining audit below.

## Product intent

Ferric should feel precise and capable to someone who uses commands and plain
configuration files, with the clarity and finish expected of an everyday
browser. Internal identifiers, serialized records, implementation terminology
and qualification machinery should not become the normal interface.

Useful technical information remains available deliberately: real command
names, configuration keys, exact security scope, selectable error details and
diagnostic reports. Presentation must not change authorization, privacy,
clipboard ownership or the meaning of a command.

This pass advances UI-003 (literal, safe display), UI-004 (native prompts),
UI-006 (readability and dimensions), and UI-007 (internal surfaces). The evidence
below is incremental; their full release qualification is not implied.

## Completion requirements

- Review everyday chrome, start/guide, switcher, settings, libraries, downloads,
  profiles/sessions, site tools, security prompts and recovery flows.
- Put human-readable meaning and next actions first. Move internal IDs, raw
  serialized data and low-level error descriptions out of primary views.
- Preserve truthful unknown/unavailable states and explicit security limits.
- Provide useful empty, loading, failure and disabled states without silently
  claiming an operation succeeded.
- Keep keyboard and pointer behavior consistent, with safe prompt defaults,
  dependable dismissal and focus restoration.
- Treat page-provided and user-provided strings as plain text. Review long
  content, narrow windows, larger fonts and primary/secondary/popup parity.
- Verify changed components interactively through Qt tests and rendered
  artifacts, then verify production resources and adapter integration. A
  component fixture alone does not prove whole-application completion.

## First implementation batch

- Downloads: filenames lead; readable binary sizes/progress replace raw bytes,
  state slugs and IDs. IDs remain in explicit details for command use. Unknown
  sizes remain unknown; paused downloads do not animate as active transfers.
- Navigation failures: explain the failure and useful next steps, preserve the
  no-replay boundary, and reveal selectable engine details on request.
- Modal input: prevent action rows from registering the dialog's navigation
  shortcuts a second time. This fixes ambiguous Escape in navigation failure
  and session-preview dialogs.
- Shared modal headers render plain text. Browser-generated security and page
  prompts show context labels rather than presenting internal event names as
  CLI commands. Real command-backed management headers remain discoverable.
- Settings: readable choice labels retain canonical submitted values; remove
  typed-schema/runtime-layer terminology from ordinary guidance.
- Whole-window tests exposed missing modal input guards and missing command
  entry points. Navigation-failure, recovery and rapid-hint prompts now block
  browser key routing. Registered `settings`, `profiles` and `sessions` commands
  open the native managers; existing list commands retain their script-facing
  behavior. IPC accepts no command payload and validates the target window.
  Settings focus their search field through the shared modal contract, replacing
  a reference to a field ID that no longer exists at the composition root.
- Switcher: readable item/action labels retain canonical action intents;
  action buttons wrap, and an empty search explains what to try next.
- Profiles and sessions: add explanatory empty states and scrollbars, show
  display names before profile handles, and wrap profile actions.
  The session manager explains private browsing and disables unavailable saves.
- Copy notices: accurately describe all clipboard content, retain hidden
  sensitive values, and use a compact height based on content.
- Secondary/popup shutdown: use the primary theme while retaining focus and
  shortcuts in the window being closed. Long warnings scroll independently
  of the safe/destructive choices; page-derived text is literal. Popup load
  completion no longer parses adjacent parenthesized calls as a chained call,
  allowing completion notification and document-end/idle scripts to run.

The focused Qt tests cover literal page text, canonical action routing,
disclosure, download progress states, and error dismissal at 900×700,
640×480 and 390×600. Optional screenshots use
`FERRIC_TEST_ARTIFACT_DIR` with the QML test runner. Artifacts live under ignored
`target/product-polish/`; they are not release inputs.

## Site tools and preview batch

Implemented after checkpoint `2f18ee2`; qualified with the manager batch below.

- Site Ledger uses readable, honest summaries and a single scrolling body.
  Unknown sharing state, private-site withholding and script matching remain
  distinct from proof of inactivity or successful execution. Diagnostic IDs,
  request decisions and raw facts are selectable behind Technical details.
- Report host disclosure and site-data consent reset on site/document changes;
  fix consent resets when the proposal changes. Actions recheck the reviewed
  diagnostic snapshot before copying, clearing data or starting/applying a test.
  A fix also verifies its current non-private origin at the Rust boundary using
  existing core state, without invoking diagnostic probes.
- Sanitized reports project experiment kind/state/duration explicitly. Live
  URLs, origin records, tab identities, commands and saved bypass lists are
  excluded. Free-form browser status text is excluded from fact statuses.
  Blocklist report metadata exposes presence booleans instead of raw
  server header values. Existing opt-in host disclosure remains separate.
- Focused controls scroll into view, including after a disclosure changes the
  layout. The shared modal frame activates focused native buttons/checkboxes
  with Enter through Qt's public button method, preserving toggle and disabled
  behavior. Shared command action labels wrap and render literally.
- Session previews show site labels, addresses and pin/mute state through typed Qt
  list properties. Placeholder tabs explain that navigation will not be
  replayed. Errors and empty reads now complete the preview request instead of
  leaving Loading active forever; unavailable previews cannot restore tabs.
- Profile deletion states that cookies and website storage stay on disk, keeps
  folder paths available for inspection, and disables confirmation while the
  folder preview is pending. Session-save drafts no longer clear on submission;
  asynchronous manager feedback still needs completion below.
- Guide/start-page deferred focus work uses view-owned timers that are destroyed
  with the view. A regression reproduced callbacks executing after unload.
  Secondary site-experiment observation now refers to the owning view explicitly.

Focused evidence: 11 Site Ledger tests, 8 preview/draft tests, and 20 start-page
checks pass. Ledger and preview renders were inspected at compact and large-font
sizes. The expanded full QML suite passes 230 tests. Rust compilation exposed
the distinction between save snapshots and validated restore entries. The projection now uses restore entries and derives readable
site labels from their safe addresses; session snapshots do not retain page
titles, and the persisted format stays unchanged. Fresh workspace, Clippy, adapter, production-surface, clipboard and IPC checks
now pass. Final evidence is recorded below; earlier failed logs are retained
under `target/product-polish/`.

## Manager feedback batch

Implemented and qualified locally; the broader product audit remains active.

- Profile writes, named-session saves and session deletions publish separate
  typed completion receipts. Pending, succeeded and failed are visible inside
  the manager. The receipt identity stays out of the interface; forms use it
  to clear only their own successful submission, preserving newer drafts.
- Failed requests retain the input and list. Friendly messages lead; selectable,
  literal storage details are available through an explicit disclosure.
  Duplicate actions are disabled while their mutation is pending.
- Catalog refreshes publish typed session names and explicit pending/error
  states. A mutation invalidates an older in-flight catalog read, which is
  discarded and repeated. Refresh preserves existing rows on a read failure.
  Reactive publications replace the profile manager's two-second polling limit.
- Both managers put their form, feedback and virtualized rows in one scrollable
  viewport, keeping keyboard focus visible at compact widths and larger fonts.
  Rename controls appear for the selected profile. Session deletion has local,
  cancellable consent that resets on navigation, refresh and new mutations.
- Preview cancellation returns to its manager; failed profile-folder checks
  explain the failure there. Confirming profile deletion returns to the manager
  to observe the durable result instead of closing before it is known.

Evidence so far: all 242 QML checks pass, including 12 manager checks for
receipt correlation, failure/retry, literal details, consent reset and compact
focus. Rendered manager panels were inspected at 390 pixels with 16-point text.
The first compiled run caught a temporary pinned-borrow lifetime in the catalog
projection; this was fixed before restarting. Logs use the `manager-*` prefix;
`manager-gate-initial.log` retains the failed run. The production integration
suite now includes actual duplicate-profile failure, catalog publication, and
an atomic session-write failure followed by a successful retry. The final production runner passes these checks, including byte-for-byte
preservation of the existing saved session after a rejected write.

The first native run identified recovery checkpoint files leaking into the
named-session catalog as `current-<UUID>` entries. Storage now excludes recognized
checkpoint names from that catalog and rejects named-session save/load/delete
paths using those reserved identities. Direct checkpoint storage and recovery
remain intact. A storage regression checks catalog filtering, exact-file
protection, recovery discovery, and continued support for names such as
`current-research`. The full storage suite passes on that change.

The native preflight also corrected an invalid bypass-site fixture (hostnames,
not origins), isolated each production-polish test's profile root, and looked
up repeater actions through the visual item tree. Site Ledger and preview
production tests pass after those corrections. The final source also uses
explicit allowlist filters in reports after Clippy suggested an unrestricted
`unwrap_or` for the earlier match syntax; arbitrary strings remain excluded.
Final validation uses `manager-*` logs, retaining earlier failed runs under
`*-initial` and `*-before-final-fixes` names.

The native render review also caught an obsolete polling refresh that retriggered
a catalog read on every completion. That polling is removed; typed publications
now update the visible list. The production regression verifies that pending
settles after completion. The final screenshot shows the useful empty state
instead of a perpetual loading indicator.

Final evidence for this checkpoint:

- Repository gate: passed; 909 Rust checks passed, 2 intentionally ignored.
- Full QML suite: 242 passed with forced-GC qualification enabled.
- Strict workspace Clippy, debug build and bounded adapter startup: passed.
- Production polish: 14 passed, including embedded resource parity, actual
  manager writes/failures/retry, site-report privacy and stale-page consent.
- Production clipboard: 81 passed in 128.25 seconds.
- Public CLI/IPC probe: passed, including native manager command routing.
- Compact and large-font manager, ledger and preview renders inspected.
- Formatting and whitespace checks: passed.

The long combined verification process was terminated during its clipboard
stage without a result. Authoritative process inspection confirmed it had
stopped; only the unfinished clipboard and IPC stages were rerun. Both passed.
The direct QML rerun that omitted the required forced-GC flag failed its GC
qualification assertion; the properly configured full rerun passes all 242.
Final logs use `manager-*`, including `manager-qml-final-qualified.log`,
`manager-native-detail.log`, `manager-clipboard-retry.log`,
`manager-clipboard-detail.log` and `manager-ipc.log`. Known teardown warnings
remain visible in the native logs and remain part of the audit below.

## Library workflow batch

The isolated baseline reproduced both off-screen Delete controls at 390 pixels
and Enter on Edit opening the bookmark instead. The current implementation:

- Places titles and addresses above wrapping actions, with literal, selectable
  full details available separately. Accessible action labels use item names.
  Journey destinations have readable labels while sending their canonical
  `current`, `tab` and `window` values.
- Restricts list navigation to focus on the list or its current row, rather
  than a child control. Qt focus scopes can have active focus while a child
  button owns the key. Native buttons now receive Enter; editable text keeps
  its own keys and modified Enter cannot open a saved page.
- Completes mark edits only after their matching durable worker receipt.
  Failures keep drafts. New catalog publications reconcile stable row identities
  instead of clearing the model on arbitrary storage completions. Successful
  mark writes request a fresh catalog, discarding older in-flight reads.
- Keeps last-readable rows on a catalog failure and exposes an explicit refresh
  with readable pending/error states. Catalog and Journey reads are tracked
  separately. A late Journey result cannot replace a subsequently selected
  library. Known recovery and storage identities remain inspection details.
- Gives libraries readable titles, honest empty states and real command headers.
  Journey rows now include their kind and node identity so keyboard activation
  follows the same path as the Reopen button. Read-only action/routing rows
  cannot fall through to quickmark navigation.
- Puts Journey search and map controls in the scrolling content, sizes map nodes
  to the active font, and keeps the top visible while its header finishes layout.
  Relationship explanations remain readable; raw transitions and source IDs
  are confined to row details and exports.

Evidence so far: 14 focused library checks and the 34 existing chrome checks
pass. The expanded full QML suite passes 256 checks with forced GC enabled.
Compact and large-font library renders were inspected; render review identified
and corrected an initially hidden Journey search header. The new production
library test exercises a real rejected SQL update, saved-row preservation,
read-error recovery, deletion, quickmark edits and Journey keyboard activation.
The compiled normal-library workflow passes, including a real rejected SQL
update, retry, failed read and recovery, deletion, quickmark editing and actual
navigation to an earlier Journey page. Interactive private testing found command
completion requesting a durable catalog in a transient profile; the common
read entry point now excludes transient profiles. The final compiled private regression passes, including actual command entry
and action-list refresh without spurious durable-worker errors. Search controls wait for their accepted
Journey query; static references keep unrelated metadata errors out of their
presentation. The QML runner now supplies its mandatory forced-GC flag itself,
so the documented repository check is self-contained. The first Clippy pass identified an unused
mutable binding and an obsolete adapter close helper after read failures became
retryable; both were removed before the current run. Final evidence for the frozen source:

- Repository gate: passed; 909 Rust checks passed, 2 intentionally ignored.
- Full QML suite: 256 passed, including 14 library checks.
- Strict workspace Clippy and bounded adapter startup: passed.
- Production polish: 16 passed in 31.82 seconds, including embedded resource
  parity, durable SQL failure/retry, read recovery, private command completion,
  late-query ownership and navigation to a different Journey page.
- Production clipboard: 81 passed; public CLI/IPC probe passed.
- Compact and large-font component and production renders inspected; formatting
  and whitespace checks passed.

The native runner links the exact static engine reported by Cargo for the
application build, recorded in `library-engine-path.txt`. A frozen copy of the
workspace engine was also used for focused private/durable workflow checks.
Earlier failures are retained: obsolete source assertions, missing test flags,
and fixtures that bypassed interactive command setup or observed transient
states after the worker had already completed. The actual private-completion
storage error was corrected at the common read boundary, rather than hidden
only in the view. Final logs use `library-gate-final.log`,
`library-clippy-final.log`, `library-adapter-final.log`, `library-native.log`,
`library-native-detail.log`, `library-clipboard.log`,
`library-clipboard-detail.log` and `library-ipc.log` under
`target/product-polish/`. The wider goal remains active.

## Prompt readability and keyboard batch

Implemented and qualified locally; the broader product audit remains active.

- The shared frame bounds its title/message viewport, preserves full literal,
  selectable messages and reserves room for choices. Forms and choices scroll
  without compressing their text. Focused controls are revealed after layout.
  Read-only inspection supports keyboard selection and caret-following scroll.
  Font metrics raise command-dialog minimum dimensions when larger fonts need
  more space. Compact prompts keep Escape visible and expose the complete
  shortcut reference through Keys.
- Initial focus is captured before transferring it to the modal. Dismissal now
  restores the prior control. Command shortcuts apply to choice navigation;
  text inspection, editable fields, checkboxes and other buttons keep their
  own keys. Return does not trigger a primary action while Cancel is focused.
- Page confirmations and leave-page prompts start on Cancel/Stay. Page response
  and authentication fields still submit with unmodified Enter. The requesting
  origin and full message remain literal and inspectable. Cancellation and
  explicit acceptance retain the existing request-resolution and lease paths.
- Certificate, passkey and sharing pickers have visible safe cancellation.
  Passkey and sharing cancellation stay outside their scrolling bodies. Account
  and source selection requires deliberate navigation; text review does not
  select an identity. PINs clear on dismissal and state changes.
- An executable fixture exposed invalid unscoped WebAuthn enum references.
  Prompt presentation and production request handling now use the actual
  scoped `WebAuthUxState` values. Production tests compare those handlers with
  the installed Qt C++ enum and exercise account/PIN/retry callbacks without
  retaining the submitted PIN. This does not replace hardware-key qualification.
- Sharing shows the source name supplied by Qt's DisplayRole, including window
  titles, with a numbered fallback when a name is absent. This follows the
  [Qt desktop-media request contract](https://doc.qt.io/qt-6/qwebenginedesktopmediarequest.html).
  Category buttons remain readable at narrow widths.
- Private-page transfer shows a literal, selectable address, clear normal-profile
  storage scope, wrapping destination actions and safe initial cancellation.
  The private-transfer completion-feedback audit remains open below.
- External-app and context-route confirmations expose full selectable addresses
  instead of truncating consent context. Rule IDs/behavior and renderer exit
  details are available deliberately behind disclosure controls. Reopening and
  rapid-hint wording explains the effect without internal descriptors or batches
  described as implementation bounds.
- Context navigation uses the destination model, reaching available choices
  outside ListView's instantiated delegate range. Unavailable contexts remain
  unavailable; selection does not bypass route validation.

Focused evidence: 37 QML checks pass across 900×700, 640×480 and 390×600, with
10-, 16- and 20-point fonts. Render review caught an oversized shortcut footer,
cramped source-category labels and misplaced scrollbars; these were corrected
before the passing focused run. Full-suite preflight passed 286 checks before
the final six external/route/recovery cases were added. Two earlier preview
fixtures sent input before their initial focus/layout settled; they now wait
for the actual safe choice or rendered form. Their focused rerun passes.
Initial compile/enum/layout failures are retained in ignored `prompt-*` logs.
The final frozen-source repository gate passes 909 Rust checks (2 intentionally
ignored) and all 293 QML checks. Strict workspace Clippy passes as well. The
23 changed source files match `prompt-source-hashes.json`; logs use
`prompt-gate-verified`, `prompt-clippy-final` and `prompt-render-reviewed`.
The production WebAuthn regression was first linked against the previous,
immutable engine: it fails because Qt's SelectAccount state was reported as
Preparing. Keyboard long-message regressions separately reproduce disabled
selection and a caret scrolling out of view before the fixes. Production
adapter/native/clipboard/IPC qualification completed below.

### Production request-ownership findings

The expanded primary-window run also caught ordinary HTTP authentication being
classified as proxy authentication: Qt populated `proxyHost` for that request.
Classification now uses `AuthenticationDialogRequest.AuthenticationTypeProxy`.
A focused source-UI preflight passes all five relevant workflows (7 checks with
setup/cleanup), including the full HTTP login and cleared credential fields.
That preflight uses an immutable compiled engine and local QML imports with
explicit native-type aliases; it is not substituted for embedded-resource
qualification. The final repository gate passes 909 Rust and 293 QML checks.


The first fresh production run passed the new page-dialog and WebAuthn state
checks and verified all 89 embedded resources. One disclosure pointer test
failed. Instrumentation showed that waiting for the first rendered frame made
it pass; the test now waits for `frameSwapped` before clicking. The WebAuthn
request stub also now supplies Qt's PIN metadata, avoiding an invalid mock-only
binding warning.

Render inspection found a real defect that the earlier response-only tests did
not detect: Qt's default JavaScript dialog was appearing over Ferric's prompt.
JavaScript, authentication and file-dialog handlers now claim request
presentation synchronously, before any deferred portal work or cancellation.
This is Qt's `accepted` presentation flag; approval still uses the existing
bounded, single-resolution accept/reject paths. See the
[JavaScript dialog](https://doc.qt.io/qt-6/qml-qtwebengine-javascriptdialogrequest.html),
[authentication](https://doc.qt.io/qt-6/qml-qtwebengine-authenticationdialogrequest.html)
and [file dialog](https://doc.qt.io/qt-6/qml-qtwebengine-filedialogrequest.html)
request contracts.

The same capture showed an unavailable origin for an ordinary HTTP page. QML
URLs do not supply the assumed `.host` property. A scalar, read-only adapter
method now reuses the existing Rust canonical-origin and permission-origin
normalization; credentials, paths, query strings and fragments are excluded.
The dialog reads the actual request origin rather than guessing from the
current top-level page. Opaque origins remain explicitly unavailable.

Production regressions now assert request ownership and the exact origin,
exercise a local HTTP 401 challenge through Ferric's credential fields, and
cancel a real file-input chooser without selecting a file. The current
28-source snapshot is recorded in `prompt-ownership-source-hashes.json`.
The ownership repository gate passes 909 Rust checks (2 intentionally ignored)
and 293 QML checks. Strict workspace Clippy also passes after documenting the
QObject receiver required by the pure origin invokable, following the adapter's
existing convention. That lint annotation is the only source change after the
gate; the current 28-source snapshot includes it. Production integration is
qualified below. Earlier native results above belong to the preceding snapshot.
The original goal remains active.

### Final prompt checkpoint evidence

- Final repository gate: 909 Rust checks passed, 2 intentionally ignored;
  all 293 QML checks passed (`prompt-ownership-gate-final.log`).
- Strict workspace Clippy: passed for the final Rust bridge and receiver
  annotation (`prompt-ownership-clippy-final.log`). The subsequent change only
  classifies HTTP/proxy presentation using Qt's explicit request type.
- Adapter startup: `cargo xtask test adapter` passed after the ownership/API
  changes. The final application binary also passed a fresh bounded 10-second
  startup with no QML errors (`prompt-ownership-binary-startup.log`).
- Production polish: 21 checks passed in 44.33 seconds, including all 89 embedded
  resource bytes/hashes, real page responses, HTTP login and cleared fields,
  file chooser cancellation, and Qt account/PIN/retry callback state handling.
- Production clipboard: 81 checks passed in 143.31 seconds.
- Public CLI/IPC probe: passed. The production runner links the exact Cargo
  application engine recorded in `prompt-ownership-engine-path.txt`.
- Final source hashes match all 28 changed source files. Compact/large-font
  component captures and the actual 390-pixel production confirmation were
  inspected; the final capture has the correct origin and no default-dialog
  overlap. Formatting and whitespace checks pass.

Final logs use `prompt-ownership-*`; the CTest detail logs preserve individual
results. Earlier unscoped-enum, HTTP/proxy and pre-render-click failures remain
in separate logs and the before-type snapshot. The final capture replaces the
earlier view that exposed the default-dialog overlap. Existing invalid-provider-context
warnings at engine teardown remain visible and require the whole-window audit.
These checks qualify the changed primary-window workflows, not physical
security keys, portal selection or secondary/popup page-prompt placement.

## Cross-window prompt and lifecycle batch

Implemented and qualified in the compiled production runner. Strict Clippy
and the final adapter startup check pass.

- Page confirmation, HTTP authentication, client identity and security-key
  surfaces mount in the requesting secondary window or popup while retaining
  their existing request owner and resolution policy.
- Close confirmation temporarily suspends the underlying surface's shortcuts.
  Cancelling close preserves the request, login draft and focused field.
  Modal focus is acquired synchronously before another prompt can capture it;
  layout retries and restoration use view-owned timers.
- Child windows release their requests while the primary creation context is
  still callable. Normal close, direct destruction and engine disposal share
  idempotent cleanup. Root-owned cleanup precedes child destructors instead of
  suppressing all runtime warnings. Popup permission cleanup matches its host,
  avoiding cancellation of another window's request on a shared UI owner.
- Secondary polling called a nonexistent file-dialog method, aborting the
  remaining runtime work. It now calls the actual window-owned method.
- Certificate identity uses typed canonical URL helpers. Qt client-certificate
  authorities have no scheme; the adapter accepts their exact authority shape
  and adds the known TLS transport before normalization. Page origins never
  use this transport-specific conversion.
- TLS decisions defer the actual engine request while the user reviews it.
  Non-overridable errors, subresources, protected hosts and unavailable origins
  remain blocked. The prompt describes session scope instead of falsely
  promising a request-only exception: Qt retains host/certificate/error
  allowances in memory. See [Qt's certificate decision API](https://doc.qt.io/qt-6.11/qml-webenginecertificateerror.html)
  and [Qt's session certificate policy](https://raw.githubusercontent.com/qt/qtwebengine/6.11/src/core/net/ssl_host_state_delegate_qt.cpp).

Current evidence: all 300 QML checks pass, including five behavioral focus
controller tests. Source-QML native tests verify prompt placement, login draft
retention and disposal with requests pending. Stress qualification reproduced
an intermittent Pass Through assertion: its interaction snapshot showed
`document_known: false` and `normal-mode-retained`. The test now waits for the
committed native document identity before expecting restoration; a readable DOM
alone does not satisfy the safety precondition. New production tests use a disposable
local HTTPS server and generated self-signed certificate, never the system trust
store. Client-certificate/security-key in-memory requests qualify window/input
ownership only; they do not establish real credential or hardware support.

The first compiled run passed all real HTTPS cases and prompt placements, but
its two security-key mocks omitted Qt's always-present `pinRequest` property.
The corrected mocks pass. The subsequent engine-disposal run exposed an actual
native crash: Qt's view-level JavaScript callback dereferenced a missing QML
engine during profile teardown. Core metadata and Qt 6.11's callback wrapper
confirm the mechanism; memory exhaustion was ruled out.

The current fix introduces a small native script runner over public
`QWebEngineFrame` callbacks. It owns bounded pending results and deadlines,
checks the owner/context before converting values, and cancels results by view
or root lifetime. No QJSValue or raw view pointer is retained in WebEngine's
callback. Existing script worlds and policy interpretation stay with their
original callers. History refresh/focus retries now use component-owned timers.
The disposal regression deliberately leaves a result pending. The rebuilt production runner passes all 40 cases, including deliberate
engine disposal with a result pending, without TypeError/ReferenceError/invalid
context warnings. The final repository gate passes 910 Rust tests (2 intentionally ignored) and
309 QML checks. Three additional native window/disposal runs pass (QtTest totals
10, 6 and 6), including immediate close without screenshot delays. All 81
clipboard checks and the IPC probe also pass. Strict Clippy passes on the final source. Its initial failure was a
documentation-only missing-backticks lint; the corrected comment is the only
post-gate source difference, verified against the source hash snapshot. Adapter
startup passes on the rebuilt final application. The preceding 910 Rust / 300 QML gate did not qualify these
later callback changes. The expanded QML suite passed 308 before
splitting cancellation and runner-destruction into separate regression cases.

Final evidence for this checkpoint:

- Repository gate: 910 Rust tests pass, 2 intentionally ignored; 309 QML pass.
- Strict workspace Clippy and final formatting/whitespace checks: pass.
- Actual application engine: 40 native product-polish checks pass, including
  all 89 embedded QML/script resources, real HTTP login, six HTTPS decisions,
  session exception reuse, safe close cancellation and pending-result disposal.
- Three additional window/disposal runs: pass, with QtTest totals 10, 6 and 6.
- Clipboard: 81 pass; IPC probe and bounded adapter startup: pass.
- Native secondary-login and popup-certificate renders inspected.

Current logs use `script-runner-*` under ignored `target/product-polish/`.
Earlier failures remain in `crosswindow-*` and `script-runner-unit-first.log`.
The qualified 37 source/test inputs have hash snapshots before and after the
one documentation-only Clippy correction; no executable statement changed
between those snapshots. The final application was rebuilt and its startup
qualified after that correction. These checks establish this checkpoint's
behavior, not completion of the remaining product audit below.

## Navigation, report and transfer batch

Implemented and qualified locally in the production application. The broader
product audit remains active.

- Visible-target search uses readable control and scope labels, full-width
  literal titles and explicit activation. Keyboard selection scrolls into
  view. Action-reference Enter inspects a row without executing its command.
- Diagnostics puts typed version, compatibility and storage summaries before
  an optional selectable report. Unknown results stay unknown. Startup version
  checks no longer create or replace a reviewed report. Saving exports the
  reviewed snapshot, and reports its result inside the panel.
- Report preview shortening preserves UTF-8 boundaries, including a multibyte
  character crossing the 48 KiB limit. The full report is retained separately.
- Native file URLs are decoded at the Qt boundary rather than calling C++
  methods on QML URL primitives. Exports, uploads, downloads and script-manifest
  selection use the same conversion in primary, secondary and popup windows.
  The boundary receives QUrl values directly: re-parsing a URL's display text
  loses filename fidelity. Remote authorities, relative or invalid URLs and
  NUL paths fail closed; Unicode, spaces, plus and percent characters remain
  exact. Export failures retain their report and expose selectable details.
- Private bookmark transfers wait for durable completion in an independent
  receipt lane. Ordinary bookmark writes cannot complete a transfer by
  accident, and multiple results in one worker poll are all processed.
  Failed drafts remain available, and choosing another page cannot relabel a
  pending transfer. Completion polling covers registered durable windows even
  when the primary window is private; transient and unavailable profiles do
  not attempt to restart a durable storage worker. Dirty session state without
  a checkpoint destination no longer schedules a perpetual 1 ms retry; its
  in-memory state is retained. Opening a destination for Add bookmark uses
  its start page rather than navigating the private address in normal browsing.
- Normal profiles with unavailable storage no longer fall through to private
  history. Compact certificate prompts use less empty space, and HTTP login
  confirmation says Sign in.

Evidence for this checkpoint:

- Full source QML suite: 320 passed on the final source. Narrow and large-font
  renders were inspected; diagnostic facts and selected search results retain
  useful room, without a scrollbar when the result fits.
- Focused production workflows: 8 passed. The broader production polish group
  passes all 45 checks, including embedded resource parity, actual upload and
  export dialog acceptance, overwrite refusal/retry, storage failure, ordinary
  versus transfer receipt correlation, and prompt/teardown regressions.
- An actual private source and new regular destination now complete Add
  bookmark, preserve the reviewed address while another request is rejected,
  and retain the source page. The destination stays on its start page and its
  history does not gain the private address. A private session also settles
  without a permanent 1 ms checkpoint retry.
- Clipboard regressions: 81 passed in 144.43 seconds. CLI/IPC manager-command
  validation and formatting/whitespace checks passed.
- Repository gate: passed; 913 Rust checks passed, 2 intentionally ignored.
  Strict workspace Clippy and adapter startup passed. The adapter uses a
  disposable profile and a bounded 10-second lifetime.
- After updating two outdated source assertions, the reduced-motion check
  verifies its actual binding and the JSON allowance records 20 parses and
  39 stringifications. Production source stayed unchanged during those
  test/documentation corrections. The report guard was strengthened and
  passed again with an explicit engine-notice refresh before first review.
- Final application-artifact re-link: 5 passed, covering resource parity,
  report review/export, actual private transfer and idle scheduling on the
  freshly rebuilt application engine (7.40 seconds).

The native failures exposed two production defects beyond the original URL
conversion: completion polling was disabled for regular child windows whenever
there was a private primary window, and an in-memory session without a durable
checkpoint path scheduled perpetual retries. Both are fixed in the passing
production tests. Earlier logs remain under `navigation-*`; they also retain
the failed intermediate string-based URL conversion and fixture corrections.
Native fixture corrections follow Qt's public dialog lifecycle: set an initial
selection, open the dialog, then call its accept method. Emitting the accepted
signal directly skips the platform selection step. The upstream behavior is
visible in [Qt's FileDialog implementation](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quickdialogs/quickdialogs/qquickfiledialog.cpp#L543).
The focused run also measured report creation at 211–244 ms; startup engine
notices no longer invoke that work. Full report creation still needs a
responsiveness review in the remaining whole-window pass.

Earlier compile diagnostics are preserved in `navigation-build-initial-*` and
`navigation-build-initial.log`; they caught two corrected Qt/Rust type errors.

## Diagnostics responsiveness batch

Opening the report previously performed external capability probes, theme-file
reads and a SQLite health check on the GUI thread. Site Ledger also built a
full diagnostic snapshot to use a few static and live Qt fields. The previous
native measurements recorded 211–244 ms for report creation.

The current implementation captures owned, in-memory and Qt facts on the GUI
thread, then collects the full report on a bounded worker. The panel opens with
“Preparing report…” feedback. Close remains available; Refresh, Copy and Save
wait for a complete report. Failures retain selectable details behind the
shared disclosure control and offer an explicit retry. Site Ledger now uses
only its required lightweight facts.

Report ownership includes the profile session and configuration revision. One
read can be in flight with one latest replacement; obsolete results cannot
authorize export or replace a newly opened report. Closing clears report
authorization. Teardown does not wait for the read-only worker. Pending review
and IPC requests have a 15-second deadline, and the IPC wait queue is bounded.
Concurrent IPC clients share collection and still receive a complete live
report. The CLI's diagnostic response deadline allows the server's 15-second
collection deadline to finish; its ordinary handshake/write bounds stay in
place. A delayed live-server regression exceeds the ordinary five-second
response timeout. A short-lived cache is separate from the exact report reviewed
for export; reading diagnostics through IPC never grants export authorization.

The private-history transfer panel now consistently says Close when dismissing
the view, including after a transfer has completed.

New worker tests cover blocked I/O, replacement,
owner/configuration changes, cache expiry, failure/retry, nonblocking teardown
and report-size limits. New production cases exercise close/reopen and settings
changes while a regular or private report is pending. The public IPC probe now
requires complete reports for concurrent clients.

Qualification on the frozen application source:

- Repository gate: 921 Rust checks passed, 2 intentionally ignored, and all
  323 QML checks passed (`diagnostics-gate.log`). This includes a real local
  socket response delayed beyond the ordinary five-second CLI deadline.
- Strict workspace Clippy passed (`diagnostics-clippy.log`). The application
  build succeeded; native tests link its frozen archive, whose original path
  is recorded in `diagnostics-engine-path.txt`.
- Focused production tests: 8 passed in 13.81 seconds, including embedded
  resource parity, exact reviewed report export, regular/private report
  close/reopen and configuration invalidation, Site Ledger and private-library
  integration (`diagnostics-native-focused.log`).
- Full production polish: 47 passed in 106.05 seconds
  (`diagnostics-native-detail.log`). The panel opened in 52–57 ms across the
  focused and full runs; complete reports were ready at 238–263 ms. These are
  observed test-run timings, not a general performance guarantee.
- Public CLI/IPC probe passed, including concurrent complete live diagnostic
  reports alongside an ordinary tab query (`diagnostics-ipc.log`).
- Adapter startup passed with a bounded ten-second disposable-profile run
  (`diagnostics-adapter.log`).
- Thirteen focused component checks passed. Loading and failure renders were
  inspected at 390 pixels with 20-point text; 640- and 900-pixel variants also
  pass. Fresh compiled library/manager captures remain readable and retain
  accessible actions and dismissal.
- Clipboard regressions: all 81 passed in 141.99 seconds
  (`diagnostics-clipboard-detail.log`).

The initial native lifecycle fixture supplied an unsupported `cancelled`
counter outcome. Its report correctly contained no recorded counter. The
fixture now uses the supported `resolved` outcome and verifies that only the
new report contains both recorded resolutions. Failed intermediate logs remain
under `diagnostics-native-focused-initial` and
`diagnostics-native-lifecycle-observed`; the earlier lint finding is retained
in `diagnostics-clippy-initial.log`. Existing Qt/GCC and offscreen WebEngine
warnings remain visible; these results do not claim a warning-free build.

## Window settings ownership batch

Status: ownership and userscript workflow changes qualified; compact layout and
cross-window input remain under audit.

Settings presentation now lives in a shared workspace with an explicit
BrowserUi owner and receiving window. The primary window uses the same
workspace as regular, private and temporary secondary windows. Search drafts,
validation failures, userscript feedback, confirmation dialogs and focus stay
with that owner. Private and temporary settings explicitly use the in-memory
scope; userscript changes remain unavailable there. The shared userscript
catalog is described as shared by regular profiles.

Secondary-window settings commands and help for known configuration keys are
enabled through the existing interactive-command capability boundary. Other
unsupported requests retain their restrictions and now explain which window
can handle them without referring to internal chrome capabilities. Secondary
fonts, palettes and command/status geometry follow that window's validated
configuration, preserving the primary window's appearance.

The settings form now uses the shared focus-aware scrolling container. Action
buttons and the session-scope checkbox wrap at narrow widths. Operation
feedback stays in a separate bounded viewport so editing does not scroll it
away. Technical errors remain selectable behind disclosure, and failed edits
retain their values. Render review caught the original clipped button and
checkbox labels, then the focused-field and feedback visibility problems.

Userscript enablement now reports pending and terminal outcomes through the
existing userscript operation-state field. A failed background change clears
optimistic overrides and restores the saved value. Retrying can complete and
publish the new inventory. A production regression blocks the installed script
directory, observes failure and rollback, restores the directory and verifies
the retried change in the actual saved manifest.

Preflight evidence includes the shared production C++ settings model in the
QML runner, 332 passing QML checks, and an earlier source-UI run against the
qualified diagnostic engine. That source-UI run passed 9 primary/secondary
surface checks, but preceded the final layout and command-capability changes.
The later production run uses the real command-bar submission path; simply
calling execute_command would have missed the unsupported-command guard.

Final checkpoint evidence:

- Strict workspace Clippy passed (`window-settings-clippy.log`). All five
  changed Rust files still match that run's recorded hashes.
- The final repository gate passed: 922 Rust tests, 2 ignored, and 334 QML
  checks (`window-settings-gate.log`). Formatting and whitespace checks pass.
- All 51 production polish checks passed on the embedded UI in 188.8 seconds
  (`window-settings-native.log`). This includes actual command-bar settings in
  regular/private/temporary windows, isolated font changes, rejected-edit
  retention, userscript write failure/rollback/retry, and removal.
- The final CLI/IPC probe passed (`window-settings-ipc-final.log`), as did
  adapter startup with a bounded 10-second lifetime (`window-settings-adapter.log`).
- An immediately repeated unchanged Cargo build reused every artifact and
  completed in 0.52 seconds (`window-settings-noop-build.*`). The earlier
  generated-manifest freshness trace did not justify changing the existing
  write-if-content-changed implementation.
- All 81 native clipboard checks passed in 221.9 seconds
  (`window-settings-clipboard.log`).

The final workflow audit reproduced another real defect: the userscript
removal confirmation was stacked behind settings, and Escape reached the
underlying surface. The confirmation now has an explicit upper stacking layer;
settings is disabled while it is open, and disabled modal surfaces no longer
register active shortcut handling. The original pointer and Escape probes both
failed; all 11 focused settings checks now pass. Final embedded application
coverage verifies pointer cancellation, Escape, and actual script removal.
Earlier intermediate evidence remains in `window-settings-before-removal-*`.

The userscript failure/retry fixture also needed correction: Qt's retry
assertion evaluates its expression again on success, so an accepted install
must be latched rather than submitted twice. Failed fixture logs are retained.

Render inspection at a real configured 14-point font in a 640×480 secondary
window shows that guidance and feedback crowd the settings controls. That
compact layout remains incomplete. The secondary input guard also still
references primary-window chrome state; independent-window keyboard behavior
and focus return need direct regression coverage and correction. These remain
part of the full polish goal. No whole-product completion is claimed.

## Independent-window input and compact settings batch

Status: qualified on the frozen source below.

Saved overlay focus now belongs to the receiving window. A dismissal removes
that window's most recent entry, preserving nested prompts and other windows'
drafts. Command-mode focus return checks overlays in its own window only.
Page, authentication, certificate, security-key, context-menu, file and renderer
prompts retain their receiving host when closing. Released windows discard
both saved and queued focus returns. Secondary keyboard routing uses its own
chrome and prompt state instead of the primary window's open panels.

Real geolocation requests from two regular profiles reproduced a permission
queue defect: identical origins and permission types were combined across
owners. Grouping now also checks BrowserUi, host, view and tab. Advancing to a
new owner retires the previous focus/input lease and starts the successor's
lease. Closing a window removes all its queued permission requests, including
requests that have not reached the front. Existing decision and persistence
boundaries remain unchanged.

Settings and installed scripts now have separate, keyboard-accessible tabs.
Search results lead the settings form; empty searches explain how to recover.
Refresh and Close stay outside the scrollable form. Compact settings use less
spacing while retaining the configured text size, shorter validation feedback,
and full selectable technical details. Keyboard help switches to disclosure
when its actual text width exceeds available space. Reopening a surface or
requesting help for a setting restores focus to the appropriate control.

Final checkpoint evidence:

- The final repository gate passed: 922 Rust tests, 2 ignored, and all 341 QML
  checks (`window-input-final-gate.log`). Formatting and whitespace checks pass.
- Strict workspace Clippy passed on this source (`window-input-final-clippy.log`).
- All 54 embedded application checks passed in 187.2 seconds
  (`window-input-final-native.log`), including resource parity, independent
  window shortcuts, cross-profile geolocation prompt separation/handoff,
  queued-window closure, and the retained settings/script workflows.
- All 81 native clipboard checks passed in 226.6 seconds
  (`window-input-final-clipboard.log`). Adapter startup passed with a bounded
  10-second lifetime, and the standalone CLI/IPC probe passed.
- Real configured-font settings checks and inspected renders cover 640×480
  at 14 pt and 390×600 at 20 pt. All 14 focused settings checks and 11 focus
  checks passed. Source hashes and application/archive digests are recorded in
  `window-input-source-hashes.json` and `window-input-final-artifacts.json`.
- The original focus failures, cross-profile permission grouping, and compact
  layout failures remain in the `*-before*` logs. Explicit routing-resumption
  checks caught a cached binding failure after prompt dismissal
  (`window-input-routing-resume-before.log`). Observable presentation state now
  drives the UI guard; the runtime lease is checked on each key dispatch.
  The final native run includes both windows returning to usable input.
- The intermediate gate exposed an outdated structural assertion for the
  former global context-menu expression. It now checks the owner-scoped
  presentation boundary and passes in the final gate and strict Clippy run.

A further production-component probe reproduced a same-window overlap issue:
Escape can fail to dismiss a permission prompt while settings remains open
underneath (`overlapping-permission-surface-probe-final.log`). A separate test
copy under `modal-input-prototype/` now passes 24 checks for visual ordering,
frontmost input, delayed focus returns, independent windows, and destruction
with visible modals. That prototype also caught late scroll callbacks and
controller-before-surface destruction; its fixes are not yet in product source.
Integrating and qualifying this modal ownership work remains required.

This batch does not finish profile/session manager access, popup management or
explicit IPC window routing. Popup windows still share an existing permission
owner; independent-profile results do not establish full popup input parity.
The whole-application, configured-font walkthrough remains part of the goal.

## Modal ownership and focus lifetime batch

Status: implemented and qualified on the frozen source.

A production-component probe reproduced Escape failing to dismiss a permission
prompt over settings. Each browser window now owns a modal input controller.
The controller follows the surfaces' actual visual ancestry, z order and
sibling order, so a high local z value inside a lower layer cannot take input
from the front dialog. Only the front surface registers modal shortcuts and
reclaims focus. The policy and meaning of each decision remain unchanged.

Queued mode/overlay focus returns respect that window's active modal. Covered
surfaces stop obsolete initial-focus work. Reactivating a surface only supplies
initial focus if focus has not already returned to one of its controls; this
preserves edited fields and drafts after the front prompt closes. Newly opened
surfaces still take focus synchronously after ownership registration.

A disposal state prevents callbacks into a modal controller destroyed before
its surfaces. Child choice shortcuts disable when their dialog is gone. The
shared scroll view explicitly disconnects external focus/cursor signals and
stops its timer during destruction. Tests reproduced both controller lifetime
and late scroll-callback defects before these corrections.

Evidence:

- The separate prototype passed 24 checks before integration. The integrated,
  dedicated ownership suite passes 13 checks, including Escape/Enter on the
  front permission prompt, preserving an unapplied edited value, actual visual
  ordering, enabled-state changes, queued focus returns, independent windows,
  and repeatedly destroying windows with visible modals.
- A stronger integration test caught initial-focus work overriding a correctly
  captured editor after prompt dismissal. The snapshot was valid; a late
  ownership focus timer moved focus back to Search. Conditional reactivation
  focus and cancellation of obsolete initial-focus work fix this regression.
- The full QML suite passes all 354 checks
  (`modal-input-qml-corrected-full.log`).
- Production source-UI preflight passes 21 checks (`modal-input-native-source.log`),
  including real geolocation requests over settings and a nested userscript
  confirmation, both Escape and safe-default Enter, preserved search drafts,
  independent windows, permissions, script feedback, window close, page
  prompts, nested restoration and requesting-window teardown.
- The final repository gate passed 922 Rust tests (two ignored) and all 354
  QML checks (`modal-input-final-gate.log`). All 58 embedded application
  checks passed in 201.9 seconds and all 81 native clipboard checks passed in
  222.7 seconds. Adapter startup and the standalone CLI/IPC probe also passed.
- The frozen engine archive SHA-256 is
  `f231fadcfea185a2e946b1c03150ed16287c8a180ebd238bc5e67f7c319f074f`;
  source and application digests are recorded in
  `modal-input-source-hashes.json` and `modal-input-final-artifacts.json`.
  Rust source and Cargo/build-script configuration are unchanged from the
  passing strict Clippy checkpoint `9d006ca`; the new code is QML and the
  native/QML regression harnesses.

The actual-font walkthrough reproduced a further compact-layout problem:
font-based spacing consumes the permission context and makes short action
labels wrap vertically. A separate layout prototype passes 48 permission,
window-size and profile combinations. Production integration, other prompt
layouts, rendered review and final qualification remain required.

The manager/popup/routing, ordinary status-copy, and whole-application
configured-font audits remain part of the original goal.

## Configured-font dialog layout batch

Status: implemented and qualified on the frozen source.

This advances UI-004 prompt context and safe interaction, and UI-006 font and
logical-dimension requirements. Actual-font renders exposed permission context
being clipped and the short Deny action wrapping vertically. The prompt
fixtures had varied font size while fixing `chromeScale` at one; using the
production FontMetrics calculation reproduced 39 failures across the prompt
suite (`dialog-layout-production-before.log`).

The shared modal frame now caps extra spacing by the available window. Text
keeps its configured font size. Action gutters and spacing use logical units
and natural text width; action height follows wrapped content. Command dialogs
reserve more of a small window for useful content, while keeping context and
safe choices reachable through their existing scroll views. Settings inherits
the shared spacing rule instead of maintaining a separate override.

Permission prompts use one context heading instead of repeating the title.
The source label says From this page. Persistence guidance follows the actual
available choices: private profiles no longer suggest that a permission can be
saved, and request-only capabilities do not imply session or future grants.
Permission policy and decision lifetimes are unchanged. Optional focus-signal
targets in the shared scroll view resolve to null when the host has no current
focus item, removing undefined-object binding warnings in lightweight hosts.

Evidence:

- All 85 prompt checks pass using actual font metrics, including a matrix of
  eight permission types, three window/font configurations and regular/private
  profiles. The matrix checks visible origin/intent, readable Deny, complete
  action labels, navigation revealing every choice, and safe cancellation.
- The full QML suite passes all 402 checks with the required forced-GC
  environment (`dialog-layout-qml-qualified.log`). The exploratory full-run
  command omitted that flag and failed only the explicit GC-availability
  assertion; the corrected run passes without the undefined focus-target
  warnings.
- Rendered and inspected permission, page-login, passkey, screen-sharing,
  recovery and generic long-content surfaces at 900×700/10 pt, 640×480/16 pt
  and 390×600/20 pt (`dialog-layout-renders/`). Oversized content remains
  scrollable; text is not reduced to fit.
- Compiled application coverage now drives real geolocation requests over
  settings and nested confirmations in all three window/font configurations,
  checking visible context, safe Enter/Escape, retained drafts and restored
  input. All 12 rows pass against the production source UI
  (`dialog-layout-native-source.log`). A machine restart interrupted the tail
  of that preflight; its remaining authentication/identity checks pass in a
  fresh process (`dialog-layout-native-source-auth.log`, eight including
  setup/cleanup).
- Final qualification passed 922 Rust tests (two ignored), all 402 QML checks,
  all 66 embedded application checks in 145.3 seconds, all 81 clipboard checks,
  bounded adapter startup, and the standalone CLI/IPC probe. Phase results and
  timings are recorded in `dialog-layout-final-status.json`. The engine archive
  SHA-256 is `bcf1def4fbfb5c73a1e1b1ef5f6e2e30df64b9087b288d227eedf02f6fbb40dd`;
  source/application digests are in `dialog-layout-source-hashes.json` and
  `dialog-layout-final-artifacts.json`. Rust source and Cargo/build-script
  configuration still match the strict Clippy checkpoint `9d006ca`.

The wider actual-font audit also found overflow in the separate shutdown
dialog. A spacing-cap prototype passes 14 ProductPolish checks and its render
has been inspected, but that change is not yet integrated. Navigation and
library fixtures pass with actual FontMetrics in the isolated audit copy.
The next batch must promote that correction and the stronger fixtures.

A disposable live IPC probe independently confirms that the CLI discards a
remote invalid-argument code and useful correction message, reporting a generic
engine failure instead (`cli-error-boundary-before.json`). Fixing the typed
forwarding boundary and ordinary failure copy remains required.

The original manager/popup/routing, status-copy and whole-window finish remain
required. Component renders do not establish complete application consistency.

## Close-dialog spacing and ordinary userscript failures

Status: implemented and qualified on the frozen source.

The separate close-window/shutdown surface now limits extra spacing by its
own host-window dimensions, even when another window supplies its theme. Text
retains the configured font. The actual-font test at 390×600/20 pt now keeps
both safe and destructive actions inside the panel, with long copy scrollable.
Product, navigation and library fixtures use real FontMetrics instead of a
fixed scale while varying their font. All 402 QML checks pass on this source
(`status-shutdown-qml-full.log`).

Asynchronous userscript install, removal and toggle failures now give short
status notices pointing to Settings. The complete worker error remains in the
existing operation state and explicit Settings details. The existing native
worker-failure scenario now verifies that the complete error reaches those
details and is absent from ordinary status. Against the prior compiled backend,
it reproduces raw `Not a directory (os error 20)` implementation text in the
status (`status-shutdown-worker-diagnostic-before.log`). This fixture does not
include an absolute path in its worker error; an initial path-specific assertion
was corrected to test the actual diagnostic boundary.

Strict workspace Clippy passed on this Rust change
(`status-shutdown-clippy.log`, original process exit zero). The final repository
gate passed 922 Rust tests (two ignored) and all 402 QML checks. All 66 embedded
application checks passed in 144.9 seconds, including the real-worker disclosure
assertion that fails against the previous backend. All 81 clipboard checks,
bounded adapter startup and the standalone CLI/IPC probe passed. The engine
archive SHA-256 is
`912d4829c279218fed09c174699e4f3cb1fd85e47f9fede558101b0e8d64d1a0`.
Source, application and phase records are in `status-shutdown-source-hashes.json`,
`status-shutdown-final-artifacts.json` and `status-shutdown-final-status.json`.

Startup, inventory and synchronous submission errors, the typed CLI boundary,
window managers and the remaining full-goal walkthrough still require work.

## CLI failure and selector boundary

Status: qualified locally; retained in the polish branch.

This advances DIAG-003 and the command/IPC target contracts. A disposable live
probe showed an invalid-window query returning `E_INVALID_ARGUMENT` over IPC
but becoming a generic `E_ENGINE` failure in the CLI. Forwarding now retains
the protocol error through handshake, command, query, focus, activation, open,
diagnostics and cleanup. The CLI reuses the server's correlation and guidance,
and maps its canonical code to the public exit status. Shared `PublicError`
replaces the CLI's duplicate startup-error translation layer.

Wire failure messages contain a concise, redacted reason. Guidance and opaque
references remain structured metadata. CLI text separates reason, state and
next action, with an accessible reference for unexpected technical failures.
Legacy peers retain their complete guidance without printing it twice; missing
legacy metadata receives safe fallbacks. Raw local diagnostic context is not
newly exposed by selecting JSON output.

Panel argument and route failures now select typed categories where validation
occurs. Other existing effect handlers retain their conservative generic
category until their own sources can supply a typed public failure. They are
not classified by matching error prose. The typed dispatch wrapper lives with
runtime IPC dispatch, keeping the existing effect-handler module under its
1,000-line cap. Its pre-context-assignment validation guard remains covered.

Transport failures retain their I/O kind, reject mismatched request IDs and
ambiguous responses, and distinguish clean EOF from an incomplete length
prefix. Timeout/closed-response messages report unconfirmed completion rather
than promising that no operation was committed. CLI chains encode and check
all local selectors before sending anything. Later server failures count
accepted requests without claiming that asynchronous effects completed or
were rolled back. Live diagnostics transport failures cannot masquerade as
successful offline reports.

A stronger regression also reproduced CLI annotation discarding a command's
profile/context selectors (`cli-selector-loss-before.log`: `work` became null).
Annotations now preserve authored selectors, merge matching CLI selectors and
reject conflicts before connecting. The live probe also checks that unknown
command-authored owners cannot navigate the default profile.

Focused evidence: all 107 application unit tests pass. Real Unix-socket regressions cover
every forwarding path, stable codes/statuses, correlation/redaction, legacy
metadata, incomplete replies, cleanup failures, admission counts and local
chain preflight. Shared IPC unit tests pass with the framing and concise-message
changes. Final formatting, strict workspace Clippy and the full repository gate
pass: 940 Rust tests passed, two opt-in tests ignored, and 402 QML checks passed.
The rebuilt production application group passed all 66 checks; the clipboard
group passed all 81 checks. Bounded adapter startup and the expanded standalone
IPC probe also passed. `cli-error-final-status.json` records each phase and
`cli-error-final-artifacts.json` records the exact Cargo-discovered engine archive
and application digests. The engine snapshot SHA-256 is
`d6c87c866db0afbb8be13d4fefd5629ab41693b35e2e4bbabd9a575027101c4c`.

Qualification corrected two test assumptions: a missing window must use the
stable `windowid-N` syntax, with malformed syntax tested separately; and the
external hint guard now maps its failure to a typed denial. The source check
now verifies that denial precedes command dispatch. Both initial failures are
retained alongside the passing logs. Neither correction changed application
behavior; the final test source passed strict Clippy and the complete gate.

The new response guard also exposed the shared deserializer collapsing a valid
successful JSON null into an absent result (`cli-null-result-before.log`). The
protocol field now preserves that distinction for every client, while retaining
duplicate-field rejection; the socket-level null-result regression exercises
the full client path. Three initial Clippy findings in the new formatter were
corrected before final qualification. Missing core state is described as Busy
only when profile bootstrap is actually pending; otherwise it retains a
conservative unavailable-window failure.

Startup/inventory/synchronous notices and remaining generic local/effect
failure projections still require review. Profile/session manager access in
other windows, popup management/input parity, explicit live-window IPC routing,
the whole-window configured-font walkthrough and final release qualification
remain part of the original goal.

## Userscript inventory and failure recovery

Status: qualified locally; retained in the polish branch.

The focused before fixture (`userscript-inventory-before.log`) reproduces a
failed inventory read displaying "No userscripts installed." The manager now
publishes independent pending, ready and error properties. Read failures retain
the last loaded rows, identify them as previous results and disable stale row
actions until refresh succeeds. Loading and unknown states cannot claim an empty
inventory. Full read errors use an explicit plain-text disclosure, while the
ordinary status gives a short reason and recovery action.

Refreshes coalesce while a mutation is running and perform a real refresh after
that operation settles. Inventory feedback cannot replace install/remove/enable
feedback. Synchronous mutation failures now retain their full detail in the
operation state, and the Settings surface reads that detail instead of copying
the shortened status notice. Reopening Settings restores failed-operation
feedback so the instruction to inspect Settings remains useful. New profile
configuration resets inventory and operation presentation.

The worker previously projected every unexpected service stop as an inventory
failure on every poll, even when an install, removal or toggle was pending.
It now retains the accepted operation kind and delivers its terminal failure
once. A real thread-exit regression covers each operation and rejects repeated
idle failures. Review also found successful script changes being projected as
failed when an unrelated manifest broke the subsequent inventory read. The
worker now preserves applied changes separately from read outcomes. Real
filesystem regressions cover installation, enablement and removal with a
malformed unrelated manifest; the application check verifies the persisted
enabled state and successful operation feedback alongside the list error.
The compiled application regression also exercises a blocked scripts
directory, retained rows, disclosure, refresh coalescing, synchronous rejection
and successful recovery using disposable profile storage. QML coverage includes
failure/loading/empty transitions, previous-row disablement, retained mutation
details and compact/large-font keyboard access; the prototype render was
inspected at 640×480/16pt and 390×600/20pt.

Final qualification passes: 942 Rust tests passed with two opt-in tests ignored,
408 QML checks, 66 compiled production application checks and 81 clipboard
checks. Formatting, strict workspace Clippy, bounded adapter startup and the
expanded live IPC probe pass. The focused real-worker application workflow and
the regular/private/temporary secondary-settings cases also pass independently.
`inventory-final-status.json` records the phases; `inventory-final-artifacts.json`
records the application, compiled test executable and exact Cargo-discovered
engine archive. The engine snapshot SHA-256 is
`e833fce28e637f65eda3a84c26494724543f218e8e012b4093b2d551f29d9485`.
Final compact and narrow renders were inspected with the details cursor actually
visible; the earlier geometry-only check could capture before scrolling settled.

Startup notices, owner-correct profile/session managers, popup
parity, live IPC window routing, the whole-window configured-font walkthrough
and final release qualification remain part of the original goal.
The first Clippy run rejected the placement of the new lifecycle test module;
it was moved to the file's end before restarting qualification. The first full
native group also exposed an old test expecting Install to be enabled while
inventory loading was still pending. That fixture now waits for readiness and
then verifies installation availability separately for regular, private and
temporary profiles. Only the C++ test changed for that correction; source and
artifact hashes preserved the passing Rust, Clippy, adapter and IPC evidence.
Both initial failures remain in the local evidence directory.

## Shared tab views and owner-bound profile/session managers

Status: checkpoint qualified; the full product polish goal remains active.

The before application probe (`window-tabs-before.log`) rejects `tab-open` in a
secondary window with a request to use the main window. Those windows previously
had one static page despite the reducer supporting multiple tabs and session
restore. The persistent view reconciler now serves both window types. Views keep
stable tab IDs through ordering changes, lazy session restoration and live
movement; navigation callbacks check their current stable owner before updating
tab state. Pin/mute metadata is read through scalar Qt methods rather than a
new JSON presentation payload. Secondary tab strips use their own configured
font, visibility and position.

Profile/session presentation now uses a shared per-window workspace. Models,
drafts, feedback, preview polling and focus stay with its BrowserUi and host.
Profile opens use the validated command path; private-origin manager opens use
the native start page instead of copying a private page address. The existing
profile lock/deletion guard remains in the worker. Preview requests no longer
share a string slot with worker results. Restore payloads have one consumer in
the tab controller, which closes the owning preview only after materializing
the views. Closed folder previews stop polling and cannot reopen their manager.
Teardown guards reject input and stop polling when an owner disappears.

Focused evidence so far: the shared controller's six QML checks and the
workspace's six checks pass. The initial compiled tab-pool integration passes
with two independent live JavaScript states, switching and closing; the same run
passes regular/private/temporary Settings. The later compiled manager integration passes create/save/replace and existing
preview/feedback tests. The expanded live-transfer probe exposed startup
navigation of an adopted page, adoption before asynchronous profile setup, and
an active-view binding left stale by in-place array edits. Transfer now waits
for profile setup, skips normal startup navigation, publishes view-pool changes,
and retires replaced renderers before late callbacks. Rollback leaves source
restoration to its caller and removes the live view before reconciling the
failed destination. Final compiled checks remain pending. The first expanded
probe aborted through the harness's script timeout; the backtrace identifies
that assertion rather than a product memory fault. A subsequent identity check
showed the empty active-view binding. A separate added test used an obsolete
manager close helper; it now calls the owning workspace. One manual full-QML
run passed 417 checks but omitted the runner's forced-GC flag and failed that
environment requirement; the final repository gate will use its required flags.
The complete pre-notification QML suite passes 420 checks; the follow-up
compiled owner/manager checks pass independent drafts, replace/append with live
views, and a private-origin profile open that leaves the private URL unchanged.
The tab test now waits for the committed document URL as well as readiness,
so the initial blank document cannot receive its retained test state. Further
checks exposed logical order versus visual layout order: a visible StackLayout
fixture fails after reorder when indexed by the logical tab position and passes
when indexed by the selected live view. Both production stacks now use that
mapping. A newly detached window removes its unused startup blank tab after
successful adoption. The expanded native checks cover visible selection,
reordering, the same live renderer through detach/return, and compact 16-point
manager renders. The first final gate passes formatting, strict Clippy and all 420 QML checks,
but fails seven Rust source assertions. Six refer to the removed static view,
primary-only manager wiring or an implementation comment. Their replacements
check the shared controller's actual owner/profile mapping, per-host developer
tools, owner-bound delete confirmation and reducer fallback path. The seventh
reveals audible/mute notifications existed only on the removed secondary view;
they now update primary MPRIS from the shared factory only for its live active
primary-owned tab.

The v3 full gate passes 942 Rust tests (two opt-in tests ignored), 420 QML
checks, strict Clippy, adapter startup and live IPC. Its compiled production
group fails three of 68 cases: live transfer and two permission-owner cases.
Native compact renders also expose a zero-size workspace child that behavioral
checks missed. The workspace now anchors its presentation child to the full
window. Six new width/font cases reproduce the missing geometry before the fix;
all twelve workspace checks and the full 426-check QML suite now pass. New native
assertions require usable modal dimensions, and corrected 640×480/16-point
profile/session screenshots were inspected (`window-manager-v4-focused-renders`).

The transfer retirement trace identifies the exact reducer/UI mismatch:
`TransferTabIn` can reuse the initial idle blank tab, so removing that tab
unconditionally destroys the adopted renderer. Cleanup now leaves the adopted
stable ID intact. Removed views retire before their model rows disappear, and
deferred removal uses an inert model. Core window IDs are local to each runtime;
transfer destination lookup now accepts the existing unique window token and
rejects ambiguous core aliases instead of selecting the first match. Actual IPC
owner routing and the public window-ID projection still remain in the full-goal
audit. The shared factory uses the existing permission handler with an explicit
requesting host, rather than calling a nonexistent helper.

The v4 compiled focused run passes all eleven checks: visible tab reordering,
retained JavaScript state and renderer identity through detach/return, one tab in
the detached window, independent manager drafts, replace/append restoration,
private-origin profile opening, regular/private/temporary Settings, permission
handoff/queued-window close, feedback and preview. No TypeError/ReferenceError
appears in that focused log. A separate native manager render run passes three
checks.

Final v4 qualification passes formatting, strict workspace Clippy, the full
repository gate (942 Rust tests passed, two opt-in tests ignored, 426 QML
checks), adapter startup, live IPC, all 68 compiled production checks, and all
81 clipboard checks. The exact selected engine archive, browser executable and
native test executable are recorded in `window-managers-v4-artifacts.json`;
all 34 frozen inputs match `window-managers-v4-source-hashes.json`. The final
native compact profile/session renders were inspected again. Runner status,
logs and renders are under ignored `target/product-polish/`. Qt header and
headless-environment warnings remain; this is not hardware/Wayland release
qualification.

Separate probes against the same production source confirm two follow-up
issues: preparing a transfer fallback writes `about:blank` into an existing
model row, and rejected transfer destinations leave accepted IPC operations
nonterminal. A component fixture also reproduces a native initializer writing
through an unowned index. Live secondary Back/Forward commands and source-close
plus GC probes pass. Older toolbar callback failures belong to hidden,
zero-height legacy controls and do not establish a visible user-control bug.
The next batch must repair the confirmed ownership and terminal-status issues.

An attempted filesystem-only probe mixed old registered QML with edited files;
it is not production evidence.

All original remaining work stays in scope: provisional-view ownership,
terminal transfer failures, popup parity, explicit live-window IPC owner routing,
ordinary startup/error copy, whole-window configured-font review and final
release/native qualification. This section does not establish whole-product completion.

## Provisional native content and transfer operation ownership

Status: implemented and fully qualified for this batch. Advances TAB-001,
UI-003 and IPC-003 without adding a command, setting or JSON presentation payload.

The compiled before probe fails in both primary and secondary windows:
`prepareTransferFallback()` overwrites the current model URL with `about:blank`
while the actual page, retained JavaScript state and history snapshot remain
unchanged. The native-content component also allows a native blank initializer
to publish through index -1 by borrowing the active tab. Native content now
requires the factory's live ownership check for metadata/history/navigation
updates and focus. It preserves a negative index, invalidates delayed history
probes when the owning UI or index changes, and rejects late unowned results.
Twenty existing component cases and six ownership regressions pass. A retired
probe result releases its pending request while discarding the facts, so a
subsequent valid owner can launch a fresh probe.

A disposable live IPC before probe admits `tab-give missing-window` and keeps
its operation at `accepted` after the destination is rejected. Destination/profile
rejection now completes the operation as failed through the existing completion
method. The permanent IPC probe requires terminal failure and preserves the
active page. The new assertion fails against the before executable.

Copied native probes linked to the qualified engine preserve all frozen source
inputs while reproducing the defects. Actual secondary Back/Forward commands
and closing a transferred page's original window followed by GC pass. The
invoked broken toolbar callbacks belong to hidden, zero-height legacy chrome;
they are not a confirmed visible-control defect. Permanent native cases now
cover command history, primary/secondary native-page traversal with retained
page state, source-close lifetime and provisional fallback model/history state.
The focused compiled run passes eleven checks, including both owners' native-page
history, live command history, transfer/source-close lifetime, both provisional
fallbacks, and shared tab/manager regressions. Live IPC passes the terminal
failure assertion. The full QML run before the final pending-probe refinement
passes 431 checks; its component suite now passes all 26.

Final qualification passes formatting, strict workspace Clippy, the full
repository gate (942 Rust tests passed, two opt-in tests ignored, 432 QML
checks), adapter startup, live IPC, all 75 compiled production checks, and all
81 clipboard checks. Embedded-resource parity passes, and the production log
contains no QML ReferenceError or TypeError. The selected engine archive,
browser executable and native test executable match the recorded hashes in
`provisional-owner-final-artifacts.json`. All nine frozen inputs were checked
before this documentation-only results update. The independent verification
record is `provisional-owner-final-verification.json`; runner status, logs and
renders use the same prefix under ignored `target/product-polish/`.

Fresh secondary/popup certificate prompts and secondary profile/session
managers at 640×480 with 16-point text were inspected. These are focused render
checks, not completion of the whole-window font/theme walkthrough. Existing Qt
header and headless-environment warnings remain; release, Wayland, portals and
real authentication hardware still need their own qualification.

The next live-owner routing probe confirms a separate defect using a disposable
standalone instance and an immutable before executable. Two ready windows
advertise the same Core window ID. The child window's unique token is rejected;
a command using its advertised ID instead adds a tab to the primary window.
Tab queries reject both selectors, and the CLI windows query rejects a concrete
selector. Records are `window-routing-before.json`,
`window-routing-query-before.json` and `window-routing-confirmed-finding.json`.
The fix must make query identities and actual command/action destinations agree,
retain destination profile/context/privacy validation, handle closed owners,
and support active/last-focused routing without an explicit-target fallback.
Ordinary new-window profile selection and popup parity remain part of this
investigation. The full original polish objective remains active.

## Live window identities and native IPC owner dispatch

Status: implemented and qualified for live IPC ownership, startup view readiness
and nested dialog focus. Advances
IPC-003/004/005 and the whole-window audit without adding a command or setting.

The previous checkpoint's standalone probe confirms duplicate window IDs,
child-targeted commands affecting the primary window, and unusable query
selectors. Live GUI runtimes now share an application-owned monotonic allocator;
isolated core runtimes retain deterministic allocation. Clones, concurrent
allocation and replacement owners have direct identity tests.

The native directory retains lifetime-safe `CxxQtThread` handles with leases
removed on close. Explicit Core IDs and existing unique owner tokens select one
owner; absent, closed and ambiguous targets never choose a default window.
Queued deliveries check the captured window and readiness again. Discarded Qt
callbacks still resolve their authenticated request. Destination command/action
validation retains source, profile and context fields. Profile matching uses
the canonical name rather than its display label. Explicit-window native new
tabs no longer fail an Active-only selector guard.

Unscoped window, tab, profile and operation queries collect live owners. Active
tab queries select one owner. Collection keeps creation order and rejects a
changed window topology. Event watermarks are allocated across owners, and
instance shutdown gates apply at admission and queued delivery. CLI window
selectors and private opt-in reach their existing typed query boundaries.

The first compiled routing stage passes six focused checks. The next stage
passes 957 Rust tests (two opt-in tests ignored) and a fresh standalone build.
Its live record (`owner-routing-stage2-live.json`) proves distinct window IDs,
correct Core-ID/token command and action destinations, global operation
observation, and actual child active/last-focused selection. It then times out
on a child diagnostic reply: secondary maintenance never flushed deferred IPC.
A separate run (`owner-routing-private-before.json`) also passes explicit-owner
native new tabs, destination policy rejection and private-tab filtering before
finding private operations in an unscoped query.

Secondary maintenance now polls IPC. Operation status/stderr now follow the
private-query configuration and explicit opt-in rule, including scoped queries
and private/temporary regression fixtures. A shared window-focus handler retains
the issuing operation owner, works in secondary windows, and no longer reports
focus success while compositor activation is pending. Focus commands carry
their operation ID. The permanent `run_window_ipc_probe.py` covers actual owner
routing, focus completion, diagnostics and private operation visibility; CI and
`cargo xtask test application` invoke it. Fresh compiled verification passes as
recorded below. Ordinary new-window profile selection, popup parity,
closed/reconfigured owner integration coverage and the original remaining
polish scope are still incomplete.

The first full run stops on Clippy documentation markup (`QObject`); that is
corrected without changing behavior. The second passes strict Clippy and all
432 QML checks, then stops on the old focus-handler source assertion. The
assertion now requires the shared primary/secondary handler and issuing-UI
operation ownership. The live focus probe remains the behavioral check.

Two independent follow-ups use the immutable stage2 archive/executable while
production inputs remain frozen. `secondary-new-window-before.json` confirms an
accepted secondary `open --target window` creates no window. The strengthened
native source-close probe then explicitly destroys the original `QQuickWindow`
with `deleteLater()`: its transferred page's WebEngine profile is destroyed too
(two test checks pass, one fails). A StackLayout binding TypeError also appears.
`profile-owner-destruction-finding.json` and `source-destroy-native-before.log`
retain the evidence. Closing and garbage collection alone did not prove native
parent destruction. Profile/interceptor/callback lifetime, final-owner cleanup
and new-window profile semantics require a coherent follow-up before release
qualification; keeping an unused source window alive is not sufficient.

The third full run passes format, strict workspace Clippy, 958 Rust checks
(two opt-in checks ignored), 432 QML checks, adapter startup, the fresh standalone
build, all ten live owner-routing scenarios and the existing IPC probe. Its
native group stops at the secondary confirmation fixture: a WebEngine script
callback times out while profile bootstrap replaces a provisional renderer.
The focused production case reproduces the same timeout.

Both ordinary window kinds now gate tab-view materialization on completed
profile bootstrap. The shared controller leaves queued restore data untouched
until it is ready, then creates only the configured identities. The new QML
regression passes, as do all 433 QML checks and all ten live owner-routing
scenarios against the fresh `profile-readiness` build. The native prompt case
advances beyond the timeout, then exposes a second defect: a close confirmation
can restore page focus beneath a still-open page dialog. The shared shutdown
dialog now checks the host's modal focus scope before applying a queued return.
A focused QML regression fails before this guard and passes after it. Native
TLS, identity, teardown, settings, permission and clipboard fixtures now wait
for completed profile bootstrap before using a child page, rather than treating
a provisional blank renderer as ready.

Final qualification (`modal-owner-v2-status.json`) passes format, strict
workspace Clippy, the full repository gate, adapter startup, the existing IPC
probe and all ten live owner scenarios. Counts are 958 Rust checks with two
opt-in checks ignored, 434 QML checks, 75 compiled product-polish checks and 81
clipboard checks. All 35 frozen inputs and the exact engine/browser/native
artifact hashes are verified in `modal-owner-v2-verification.json`. Fresh close
and secondary-confirm renders were inspected. Qt/GCC and offscreen environment
warnings remain; this does not qualify release, hardware or native Wayland.
Only this implementation log changes after the verification to record results.
The original polish objective remains active.

The corrected isolated lifetime probe receives a native download signal after
experimental reparenting and forced source-window destruction, but no download
record reaches the destination owner. Its normal test failure is retained in
`source-destroy-callback-observer.log`; the earlier callback experiment instead
aborted by reading an undeclared pending-download property and did not establish
callback routing. Secondary download handling also references three pending
state properties that are absent from that window. These findings remain open
alongside resource creation-context lifetime, native ownership and final-owner
cleanup. The experimental reparenting exists only in the ignored probe harness.

## Application-lived profile resources and native teardown

The stage records below retain the intermediate findings and failures. The
shared-host checkpoint below qualifies the corrected implementation. No
installation or release qualification is claimed.

Production profile resources now use an application-created context and native
consumer leases. Ordinary views retain their QObject owner separately from
visual parenting; pending windows cannot supply policy authority. Retirement
clears view models and pool entries before deferred native deletion. Request
handlers live with their profile resources, resolve download ownership through
the tracked view, and retain the accepted owner through completion/cancellation.
Secondary windows now declare pending download state and own their chooser
handlers rather than leaving them on a source-bound WebEngineProfile object.
Regular windows using the same canonical profile and storage base reuse its
native resource group; private and temporary groups retain explicit sharing.

The strengthened permanent source-close regression explicitly destroys the
original QQuickWindow. All three regular/private/ephemeral rows pass on stage1:
JavaScript state and an unsubmitted form draft survive, the actual destination
window presents and cancels its download chooser, and the profile disappears
after the final native consumer closes. The existing production group passes
77 checks, and all ten live IPC owner checks pass. These are focused stage1
results, not a completed qualification of the current batch.

A new popup-opener regression then reproduces a remaining defect: destroying
the opener releases the popup's profile and leaves stale UI references. That
case is intentionally retained as failing until popup backend/permission and
native-view ownership are integrated. Popup consumer tracking, opener-independent
backend state, developer-tools edge cases, new-window/profile inheritance and
operation completion remain required.

Removing an unused duplicate profile factory exposes a separate stage2 teardown
crash. Coredump PID 380285 shows QQuickWebEngineProfilePrototype destruction;
Qt's prototype retains a scoped owner of its profile, conflicting with the new
lease's separate ownership of that instance. The native lease now retains the
factory and parents its profile beneath it, deleting pages before the factory.
The primary resource group supplies its existing prototype explicitly. Eleven
native unit checks pass, including the added factory-ownership case. Fresh
stage3 real-WebEngine validation passes all three ordinary lifetime rows (five
QtTest checks), including final factory/profile cleanup. The crash report and symbolized
stack are retained under `profile-resources-stage2-core-*.log`; the extracted
core copy was removed after inspection. Production code uses public Qt APIs.

### Initial popup runtime and integration findings

The initial implementation gave a native popup an independent typed runtime
before synchronously accepting its WebEngine request. The application inherits
an already validated profile, configuration, context snapshot and shared profile
lock; it starts metadata work on a worker. A small native bridge checks QObject
types, threads and distinct owners before the Rust handoff. The existing reducer
transfer events move the admitted popup target out of the opener. The popup
retains the same native profile through the application resource registry and
no longer falls back to the primary backend when its opener is destroyed.

Blocking policy now publishes an explicit readiness fact. Resource groups retain
their installed snapshot until a replacement owner has a complete policy, and
updates publish atomically. Download destination preferences use the requesting
window's typed preferences. The desktop portal audit confirmed that capability
results intentionally come from one application-wide probe; that routing is
preserved.

Evidence for the frozen `popup-owner-stage3` production artifacts:

- The focused native run passes 13 checks. All regular/private/temporary popup
  cases forcibly destroy the opener and its backend, preserve JavaScript and
  form state, retain installed policy while its replacement is unready, update
  the popup's address, present a real popup-owned confirmation and download
  chooser, and destroy the final native view/profile/backend. Ordinary transfer,
  popup userscript, permission placement and cross-window clipboard cases also
  pass in this run.
- The complete clipboard group passes 81 checks. The larger product-polish
  group passes 76 and fails four: both popup prompt cases lack a known document;
  a named session save is rejected; and a private bookmark destination reports
  completion before its native start page is visible.
- An isolated rerun reproduces both popup document failures. Session saving and
  the private transfer pass in isolation, so their broad-run failures remain
  timing defects requiring investigation. Do not discard them as load noise.
- A separate production command probe accepts `open` in the popup's runtime,
  but its WebEngine URL remains unchanged. The popup work timer does not consume
  engine actions. Command, document observation, keyboard and management parity
  therefore remain incomplete. Reuse the ordinary window host and controller
  rather than adding another partial action dispatcher.
- Strict workspace Clippy passes on the frozen production source. The repository
  gate passes 434 QML checks and the native lifetime CTest target, then fails
  eight source-contract assertions expecting the old inline profile ownership.
  Updated contracts include the new resource components and preserve profile,
  privacy, typed-policy and native download-owner assertions. Their first rerun
  passes 472 with one remaining obsolete assertion; that assertion is now
  corrected but has not been rerun. These test-only edits do not change the
  production artifacts used by the native checks.

Logs and exact artifact/input hashes are retained under `target/product-polish/`.
These stage3 failures were required follow-up work, resolved and requalified in
the checkpoint below. Popup journey provenance/session permission sharing,
developer-tool lifetime edges, ordinary new-window profile inheritance and the
original polish audit remain in scope.

## Shared popup host, session saves and page-dialog checkpoint

Qualified locally against 44 frozen inputs in
`target/product-polish/lifetime-popup-session-v5-source-hashes.json`. The
checkpoint preserves exact browser, engine archive and native test executable
hashes in `lifetime-popup-session-v5-checkpoint.json`. This records the current
batch, not completion of the whole product-polish goal.

- Popups now wrap `FerricBrowserWindow` instead of maintaining another page,
  command dispatcher and set of request handlers. A native request is accepted
  synchronously into a blank view using the exact inherited WebEngine profile
  and an independently owned typed runtime. Ordinary document observation,
  command entry, settings, managers, tabs, downloads and prompt handling are
  shared. Registration publishes the popup's own owner identity once.
- Permanent native coverage opens a real popup, navigates it through the
  command path, observes its document identity, enters and dismisses command
  mode with real keys, opens its settings/managers and creates multiple tabs.
  A nested popup keeps its native profile and JavaScript state when its opener
  is explicitly destroyed. The regular/private/temporary lifetime cases also
  retain form drafts and policy, route dialogs/downloads to the surviving
  window, and verify final native cleanup.
- Named-session saves use the existing application storage scheduler. One
  explicit save may queue behind an automatic checkpoint; repeated explicit
  requests remain bounded. Typed storage tickets correlate completions, and a
  checkpoint generation prevents an older write from clearing newer changes.
  Successful automatic saves no longer flood the status bar.
- A permanent SQLite writer-lock regression reproduces the old busy rejection,
  then verifies that the queued named save succeeds with the URL captured at
  submission even if the current tab changes before storage becomes available.
  Private bookmark transfer now waits for host profile bootstrap as well as
  backend readiness before reporting completion.
- Page requests use readable titles, including “Confirm this page's request”
  and “Sign in to this site”. Site/proxy messages remain literal, origins stay
  visible and safe cancellation is preserved. Dialogs fit short content while
  retaining bounded scrolling for long content and large fonts. Final real
  popup confirmation and sign-in renders were inspected: fields and actions
  are visible, with no surplus body space or unnecessary short-header scrollbar.

Validation on the final source: formatting, strict all-target workspace Clippy
and the complete repository gate pass; 960 Rust tests pass with two existing
ignored tests, and 435 QML checks pass. The native lifetime CTest target passes.
The production integration group passes 82 checks, the clipboard group passes
81, and all ten live IPC owner checks pass. Adapter startup passes. The final
native render run passes four QtTest checks, including setup/cleanup. Earlier
gate failures from obsolete proxy-title and duplicated-popup status assertions
are retained in the logs; updated contracts check the actual title and both
shared status call sites.

The separate `new-window-v5-before.json` probe against the same preserved browser
still fails: an unspecified profile becomes `secondary` instead of inheriting
`default`, and a secondary owner's accepted new-window action creates no third
window. This was the next integration fix, addressed by the following batch;
the earlier passing checkpoint did not cover it. Shared child-window hints
remain unavailable. Popup
journey/permission sharing, developer-tool lifetime edges, ordinary status/help
copy and the configured-font/theme whole-window review remain open. Offscreen
software rendering does not qualify native Wayland/GPU behavior, physical
credentials or real-account authentication. No installation or publication
was performed.

## Window creation and inheritance batch

Status: qualified locally for the window-creation and routing behavior below.
The full product-polish goal remains active.

The exact checkpoint browser reproduces two public-IPC defects in
`new-window-v5-before.json`: an omitted profile becomes `secondary`, and a
secondary owner's accepted new-window request creates no window. New permanent
native rows also fail against that old engine: the primary loses its context,
and the secondary never creates its requested destination.

The implementation now resolves an omitted profile from the addressed owner,
uses one native action consumer in every window, and carries an explicit
inheritance decision separately from profile-name matching. An explicitly
selected durable profile therefore stays distinct from a private profile with
the same name. Same-profile children and popups share validated typed profile
admission, retain configuration and native resources independently of their
source, and preserve temporary invocation identity. A fresh private-window
request still creates a separate off-the-record profile.

Context membership is assigned through the reducer without recursively invoking
context entry. Explicit context restoration retains its existing behavior.
Routing prompts publish their ready signal after all prompt fields, belong to
their requesting host, and retain a new-window destination through both choices.
Private-window opens and explicit profile selection bypass automatic context
routing; private and temporary browsing never use saved-context routing.

Permanent native coverage now includes primary/secondary/popup sources,
regular/private/temporary sessions, explicit durable selection with colliding
names, configuration/context/native-profile preservation, source destruction,
blank-window creation and fresh private sessions. Routing cases use actual
safe-choice and confirmation keys. The live IPC probe also verifies secondary
window creation and inherited private-query exclusions. These additions pass
on the qualified production inputs described below.

The first current-build probe exposed a readiness defect in the new admission
path: profile publication starts asynchronous network-policy work, so calling
the host's completion method immediately made context assignment reject and
retire the new window. Ordinary inherited windows now wait for backend readiness
through the existing bootstrap timer. Native popup request acceptance remains
synchronous, with its inherited profile policy retained by the resource group.
The missing-owner regression now handles a retired target safely and preserves
the admission status in its failure output. The initial SIGABRT was the test
harness's deliberate `qFatal` for an invalid QML lookup; core PID 502351 records
that call chain, rather than a native profile teardown fault. No extracted core
copy was created. Regular child and popup cases now also exercise an alternate
disposable storage base, requiring native and Rust profile paths to stay aligned.

An intermediate focused run passed 17 checks and failed three. Primary context
inheritance, secondary regular/private/temporary windows, all explicit durable
name-collision cases, primary routing decisions and popup command handling now
pass. The regular popup row reused the secondary row's alternate profile root;
fixture roots and routing config directories are now unique per data row.
Secondary route prompts had the correct input host but retained the primary
visual parent. The dialog now binds its visual parent to the requesting host's
content item, and native tests explicitly check the action control's window.
Both corrections pass the fresh focused and full qualification below.

Final evidence is recorded in `target/product-polish/new-window-v4-status.json`
and `new-window-v4-checkpoint.json`. Formatting, strict all-target workspace
Clippy and the complete repository gate pass: 960 Rust tests pass with two
existing ignored tests, and all 435 QML checks pass. The native lifetime CTest
target passes. The focused native group passes 20 checks, the broad
product-polish group passes 99, the clipboard group passes 81, and the live IPC
probe passes 13 checks. Adapter startup and all six native render checks pass,
including setup/cleanup for the four captured request cases.

The final gate initially rejected an obsolete source assertion for the old
duplicated context-window handler. Its replacement checks context-action
detection, payload arity and the shared restoration flag. That is the sole
`cfg(test)` difference from the preserved v3 production/native build; every
production and native test input, all binary hashes and the working CTest
executable were verified identical before reuse. The corrected Rust tests and
all runtime groups were then rerun. The manifest retains 50 frozen inputs.

All four final captures were inspected. Popup confirmation/sign-in panels fit
their content, preserve origins and show unclipped fields/actions. Primary and
secondary routing prompts appear in the requesting window with the normal
choice focused. The routing card still has excessive vertical space between
short sections; `routing-render-followup.json` records this remaining polish
work rather than treating the whole interface as finished.

This checkpoint verifies native profile/configuration/context inheritance and
the tested window/routing paths. Continuing session-permission sharing, popup
journey ancestry, child hints/pointer chrome, child clean-link previews,
simultaneous routing prompts, startup/status/help copy and the full configured
font/theme/compact walkthrough remain open. Native Wayland/GPU, portal,
real-account and physical-credential qualification is not established by these
offscreen software checks. No installation or publication was performed.

## Audit status and remaining implementation

These remain part of the original goal, rather than deferred release promises:

1. **Site Ledger / Site Doctor / diagnostics.** The ledger layout, readable
   summaries, disclosure and consent changes above pass fresh compiled
   qualification. The remaining diagnostics/export surfaces now have readable
   summaries, explicit report review, honest unavailable states and production
   export evidence above. Report collection now runs off the GUI thread and
   passes the compiled ownership, export and responsiveness checks above.
   The baseline 900×700 render
   (`site-ledger-before.png`) is retained to compare the overflow fix.
2. **Profile/session management feedback.** Completed and qualified in this
   checkpoint: typed previews, deletion pending state, durable failures,
   catalog refreshes, idle settling and draft retention all have actual worker
   evidence above. Retain these regressions during subsequent changes.
3. **Start, guide, libraries and navigation tools.** Audit labels, metadata,
   empty states, long titles/addresses, command explanations and learning
   affordances, including visible-target search and hint feedback. The library
   workflow defects reproduced by `library-before.log` are now fixed and
   qualified above. Continue the remaining reference/help wording and
   unavailable-state audit. Read-only action affordances, normal
   metadata-unavailable history, private-transfer completion, visible-target
   search and action-reference behavior now have compiled evidence above.
   Retain those checks through the final whole-window walkthrough.
4. **Status/error messages.** Audit operation IDs, stale-target terminology,
   adapter/engine wording and internal state transitions in ordinary status
   notices. Preserve useful diagnostic detail without flooding normal use.
   Completed-transfer dismissal now consistently says Close. Continue the
   unavailable-storage recovery and ordinary status-message review, including
   remaining "stale" and "worker unavailable" messages.
5. **All prompts.** Shared overflow, safe activation, request ownership,
   origin display and scoped WebAuthn defects are fixed and qualified above.
   Actual cross-window page/authentication placement, login drafts, focus and
   self-signed HTTPS decisions now pass production checks. Client-certificate
   and security-key fixtures establish window/input ownership only; real
   credentials, hardware keys and native portals retain separate qualification.
   Finish the whole-window compact/large-font review and the remaining prompt
   copy/layout review alongside the final walkthrough. The compact certificate
   and HTTP login wording corrections above now pass compiled verification;
   certificate renders were inspected again on the final batch.
6. **Whole-window finish.** Verify theme/readability, larger fonts and
   primary/secondary/popup consistency in the final walkthrough. The lifecycle
   investigation above found and fixed real context callbacks and a native
   teardown crash; repeated qualification and the checkpoint gates pass.
   Shared settings and profile/session manager access and ownership are
   qualified above. Popup management and explicit IPC window routing remain
   part of this audit. Check whether primary-window surfaces affect
   another window's key routing, and use actual configured font sizes in the
   final compact walkthrough so metric-based padding is exercised.
   Page/authentication
   and identity prompts now mount in the requesting child window while their
   existing request owner resolves the decision. Review remaining navigation
   tools and global feedback before claiming whole-window completion.
7. **Final evidence.** Run repository-required gates, compiled production
   integration and a rendered whole-application walkthrough after the final
   source is frozen. Keep the installed browser and normal profiles separate
   from disposable test fixtures.

## First checkpoint evidence

Later batch records above describe subsequent changes and validation.

- Workspace repository gate and Rust tests: passed after the manager command,
  route-validation and input-guard fixes. Strict workspace Clippy: passed.
  The subsequent window changes are confined to QML and integration tests.
- Full QML suite on the window batch: 210 passed. The latest focused suite:
  14 passed, including literal untrusted text, canonical submitted values,
  download disclosure/progress, private-session availability, safe dismissal,
  long-warning scrolling, uncompressed text and scrollbar placement.
- Rendered components inspected at 900×700, 640×480 and 390×600; the shutdown
  warning also uses 20-point text. Render review caught and corrected text
  compression and scrollbar placement that the initial geometry test missed.
- Fresh debug production surface group: 11 passed, including all embedded
  resource bytes, actual manager command entry/focus/dismissal, safe keyboard
  and pointer cancellation in secondary/popup windows, and real installed
  document-end/idle popup scripts. Settings and both close prompts were rendered
  and inspected. The final scrollbar correction also passes a fresh compiled
  resource-parity and production surface run (11 passed, 16.7 seconds).
- Production clipboard regression group: 81 passed in 128.5 seconds.
- Standalone public CLI/IPC probe: passed with a disposable profile. Manager
  commands accept no arguments, reject missing windows, and preserve the active
  page. This probe and the production surface group are now included in CI.
- Adapter startup smoke: passed on the final QML source (bounded 10-second lifetime).
- Formatting and whitespace checks: passed.

The first whole-application run exposed missing modal guards and manager entry
points that component fixtures could not detect. Later compiled verification
also rejected an intermediate close-dialog edit with an invalid QML property;
that edit was corrected before the passing production run above. Logs under
`target/product-polish/` retain both failed and successful evidence.

Remaining startup/teardown warnings are recorded under the whole-window audit;
passing checks do not establish that every warning is harmless. The installed
browser, normal browsing profiles and remote repository have not been changed.
