# Historical product-polish checkpoints — 2026-10-15

Historical record only. Earlier pending/failed checkpoints below do not describe
the current completion state. See the [current implementation report](product-polish-2026-10-15.md).

Current checkpoint (2026-10-18): integrated download transport and save work is
under qualification. The current source QML passes 625 checks. The preceding
file-finalization checkpoint passes strict application/adapter Clippy, 509
adapter tests (one ignored), and all five actual held-file-sync cases: normal,
Cancel, owner retirement, private completion and destination collision. Its
complete native Downloads run reproduces the three pre-existing transport
failures: cancellation and navigation before headers, and HTTP authentication.
A replacement explicit-download stream is now integrated; its isolated Qt
research passes 20 checks, including actual service-worker control of an ordinary
page and its absence from the browser-owned document in regular/private
profiles. These research results do not establish production integration.
Fresh Rust, compiled native, public IPC and final whole-window checks are
running or pending. All seven original areas remain in scope. Nothing has been
installed, committed or published as part of this completion pass. Historical
evidence below is scoped to its recorded source and artifacts.

Permission reset now identifies owners through the application's live profile
session, with a checked read-only native bridge. Matching only the origin
previously reloaded unrelated profiles while omitting popups and background
tabs. All loaded matching pages are now enumerated across ordinary and popup
owners. Different roots and independent private sessions remain separate, and
outdated view-owner references are rejected. Queued prompts and notification
closure use the same scope. Active notification owners are retained until close
instead of being silently discarded after a fixed count.

Actual command-bar reset works in primary, secondary, private and popup owners.
A private reset also reloads the page when no remembered decision exists, so
once-only access is not excluded merely because no saved rule was found.
Combined camera/microphone removal retains the preceding atomic session-store
fix. Native checks cover actual queued geolocation isolation; notification
closure uses simulated notifications with real typed owners. Physical camera,
microphone and portal revocation are not established by these fixtures.

Twenty-seven ordinary status notices now explain changed pages/tabs/windows
and useful next actions. Download activation preserves uncertainty when its
result cannot be confirmed, and editor write-back accurately says edited text
was not applied after a page change. The earlier developer-tools ownership,
interactive window creation, literal labels and fitted preview work remains
qualified by `refined-owners-v2` (commit `4a39de6`).

The preceding `permission-scope-v1` checkpoint passed strict workspace Clippy
and the repository gate: 973 Rust tests
passed, two ignored, and 492 QML checks passed. Focused preview checks pass 17;
focused production checks pass 25. The broad browser group passes 158 and
clipboard passes 81. Adapter integration and nested Wayland startup pass.
Editable-page Wayland input remains unqualified because the compositor lacks
the required virtual-keyboard protocol. Four current preview captures were
reviewed, retaining the preceding 20-capture whole-window/prompt evidence.
Frozen source/artifact hashes and complete results are retained under ignored
`target/product-polish/permission-scope-v1-checkpoint.json`.

The context reload integration now passes the four reproduced native failures:
changed labels, new context choices, removed definitions and new routing rules
publish with the reloaded settings. The application reserves configuration
publication while the validated context job updates disk off the Qt thread.
Rejected context documents, unknown profiles and invalid engine selectors keep
the preceding configuration and generated metadata intact. Corrected documents
recover in the same window, and existing windows receive the committed catalog.

The preceding overlapping-edit repair remains covered by the controlled native
FIFO case: a later edit made while the loader is busy is applied without a third
edit. The watcher preserves pending work and unacknowledged source changes.
Atomic context storage retains identity/membership, rejects unsafe affinity
changes and avoids unnecessary writes.

The current checkpoint passes strict workspace Clippy on identical frozen
source, the complete repository gate (995 Rust tests, two ignored; 492 QML
checks), production build/resource parity, 11 native context-reload checks and
34 focused window/configuration/permission-owner integration checks. The native
context suite is now part of CI. Source and artifact hashes and the complete
results are retained under ignored `target/product-polish/context-qt-v5-*`.
The initial shared Cargo output directory caused a doctest artifact collision;
the complete gate passed after separating the worktree build directories.
No installation or publication was performed.

A follow-up consistency repair is isolated at
`/home/tom/.codex/worktrees/context-consistency/RustBrowser`. Three additional
reproductions exposed offline deletions returning after restart, delayed worker
results replacing newer catalog changes, and invalid logical window bindings.
The repair adds durable commit ordering, configured-identity provenance,
atomic binding cleanup and normal checkpoint/IPC observation. Its 294 selected
core/storage/application tests and strict linting pass. Native Qt integration,
restart/deletion journeys and the combined-source qualification remain open;
these unit results do not establish full browser completion.

Durable permission readiness, remaining lifecycle/command/help wording and the
final whole-window walkthrough remain part of the original goal. Earlier
checkpoint evidence and the complete original requirements follow.

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

## Saved permissions and request admission follow-up (17 October)

The audit reproduced an older saved denial disappearing behind the library's
1,000-row display limit. Snapshots now report whether they are complete, and
the application checks the exact site before trusting an omitted rule. The
point read uses the existing index. Live requests and the Site Ledger share
that authority, including expiry, mutation admission, completion and
cancellation. Metadata eviction retains decisions associated with live writes;
completed writes cannot pin an unbounded cache.

Native qualification exposed two additional integration defects. Qt supplies
security origins with a root slash; rejecting that representation caused a
configured Allow to bypass the saved site rule. One root slash now normalizes
to the same exact origin. Unmatchable origins cannot inherit Allow, and an
explicit Deny default is preserved. Fresh window owners also initialized
invalid `{}` override metadata, so their supplied settings were rejected and
replaced by defaults. They now serialize typed, versioned empty overrides.

Unknown saved policy waits for at most two seconds without acquiring a modal
input lease or moving focus. Resolution is bound to the requesting window,
active tab and document. Navigation, transfer, retirement and owner closure
invalidate deferred requests. An unavailable lookup requires explicit consent
and shows a plain-language explanation. Combined camera/microphone policy is
resolved in the application. The prompt offers session saving only for
supported origins. Request limits include entries added to an existing group;
cooldowns belong to their real window/view owner. Background completions do
not restart a visible prompt's expiry. The local scalar association key is
explicitly documented in the QML JSON inventory.

Popup durable completions now use the existing combined live-window registry.
Their initial page and saved choices can persist without opening a manager,
while completion feedback remains with the popup. Popup journey ancestry is
still a separate confirmed defect.

The frozen `permission-refinement-v12` checkpoint passed strict workspace
Clippy and `cargo xtask check --locked --offline`, with 1,034 Rust tests,
495 QML checks and two existing ignored Rust tests. Its production engine
passed 28 native checks, including actual saved Deny and exact-absence Allow
cases, window settings, popup persistence and pending request lifetime/limits.
Native regressions inspect `QWebEnginePermission` state and prompt visibility,
so location-provider availability cannot produce a false policy pass. The
focused prompt suite passed 106 checks (already included in the 495 total).
The six subsequent full-window renders exposed switcher layout defects; they
are recorded as visual failures despite the render harness completing.

The full goal remains open. Separate stress checks reproduced incomplete
canonicalization of equivalent authorities and acceptance of a malformed
host. Their failing regressions remain isolated under
`permission-authority-before-tests.log`; changing those keys requires a
legacy-data migration and deterministic conflict handling. Also outstanding:
permission loading/unavailable presentation and notification deferral audit,
popup ancestry, configured-context deletion/reload guidance, remaining ordinary
help/status copy, whole-window configured-font/theme review, and final combined
native/clipboard/adapter/bounded Wayland qualification. These changes have not
been installed, pushed or published.

## Switcher compact layout follow-up (17 October)

Actual 20-point application renders exposed crowded shortcut/count metadata,
a search field extending outside its card, and a clipped context action at
640×480. The result row now follows its content height instead of a five-row
minimum with stretched vertical gaps. Selected titles wrap to two lines;
truncated titles and metadata have bounded, plain-text hover details. A
vertical scrollbar exposes longer results. Keyboard focus scrolls wrapped
actions into view, and selecting their row preserves focus on the action.

Scoped lists now identify their contents in the title and search prompt;
result counts use singular/plural correctly. Redundant shortcut text has been
removed from the search row; the shared footer retains keyboard help. The
empty state is centered in the viewport rather than the list's content item.

The focused suite passes 15 checks, covering 20-point 900×700, 640×480
and 390×600 layouts, literal titles/tooltips, pointer action dispatch, keyboard
navigation, wrapped actions, selection/focus agreement, optional metadata,
empty states and an unknown scope. Long-title hover stress tests also verify
that text stays within its popup and follows the owner's dark/light theme.
Tooltips now have a line and height limit, with elision; metadata no longer
starts with a separator when a profile label is absent. An isolated copy of the old switcher fails the primary-action
visibility check at 640×480. The source-frozen production checkpoint passed strict workspace Clippy,
`cargo xtask check --locked --offline` (1,036 Rust tests, 510 QML checks and
two existing ignored Rust tests), 174 native product checks, the complete
clipboard suite and 23 context/window integration checks. All six embedded
20-point dark/light renders at 900×700, 640×480 and 390×600 were inspected;
the primary context action fits the actual viewport in each.

Evidence uses `switcher-refinement-v3` for the successful gate/build/native
phases and `switcher-refinement-v6` for the final render review. The render
harness now waits for refreshed results and searches the visual item tree for
delegates. Earlier assertion/readiness/lookup failures and a transient empty
snapshot are retained rather than counted as completed visual checks. V6
revalidates every frozen production input and the exact engine/native hashes;
production source did not change between those checks. Layout at configured
fonts is checked at runtime instead of requiring the old oversized row minimum.
The transient no-results message during a query refresh remains a separate
loading-state issue in the broader pass.

A separate backend counterexample showed that a normal profile with failed
storage claimed a complete empty set of saved permissions, just like a
private session. The application now keeps that absence unknown; only
private/temporary sessions can establish they have no durable choices. Its
24 isolated permission tests and strict application Clippy passed before the
scoped repair and two regressions were integrated. The broader Site Ledger
availability model and pending-notification retention remain open; two
isolated presentation/function counterexamples reproduce them.

## Saved permission availability and notification delivery (17 October)

Saved permission metadata now distinguishes ready, loading, unchecked,
unavailable and deliberately not stored. A failed read no longer leaves the
Site Ledger claiming it is still checking indefinitely. Exact-read failures
have a bounded retry delay, and a failed refresh preserves previously proven
rules. The existing readiness boolean remains compatible; the additive status
is documented in the QML contract inventory. The integrated application suite
passes 70 unit and four integration checks; the focused Site Ledger suite
passes 14 checks. These do not replace final native failure/recovery validation.

A disposable native counterexample confirmed that explicit **Allow once**
granted Qt permission and created a real notification, but Ferric then closed
it because saved policy correctly remained Ask. The controller now accepts
that native authorization while continuing to enforce a current Deny. It
retains window/profile/private-session ownership through deferred policy and
portal work, closes evicted requests, and rejects late acknowledgement or
clicks after ownership changes. Private notification tags include the actual
private session identity. A service error reports cancellation without claiming
that a nonexistent Qt fallback displayed anything.

The native transport now treats submission and acknowledgement separately.
It resolves the unique service owner before capability discovery and delivery,
validates reply signatures and positive desktop IDs, and acknowledges the
page only after the service accepts the request. Plain notification text is
escaped when a desktop service advertises markup support. Required-portal mode
uses AddNotification/RemoveNotification rather than merely probing the portal
before sending through a different service. Desktop dismissal and webpage
closure both release the matching live record; click dispatch also requires
a live owner in the controller.

Cancelled sends retain a bounded transport slot until their reply is consumed,
so a late remote ID can be withdrawn without redisplaying the notification.
The delivery deadline is five seconds; pending bus operations are capped at
32 and active records at 128. Cancellation and daemon replacement use the
unique destination, protecting a new daemon that reuses an old numeric ID.
The portal cannot report whether the user actually saw or dismissed a message;
service acceptance is the evidence behind the word “sent.” If the desktop
protocol never returns an ID before its hard bus timeout or the process exits,
there is no protocol operation to retract an unknown desktop ID. Tests do not
claim that stronger guarantee.

The focused controller suite passes 31 checks. The separate transport suite
passes 19 checks, including actual asynchronous bus replies, cancellation,
source destruction, deadline expiry, malformed responses, sender validation,
daemon replacement, both transports, bounded queues and sustained traffic around
an older delivery deadline. Its fake service runs
out of process: Qt method replies do not retain sender metadata, and local
connection shortcuts are unsuitable evidence for this boundary. The private
bus configuration has no activation directories and cannot start the user's
installed desktop services. These checks and the native browser notification
suite are wired into the repository's application CTest targets and CI.

The first full native run confirmed real Allow once delivery and show/click
callbacks, plus successful service-failure, webpage-close and owner-close
cases. It also exposed two window-activation paths calling an Item-only focus
method on a Window. Those calls are removed; normal window activation retains
Qt's existing focus target. The corrected native test fails on QML type and
reference errors and waits through deferred activation. Fixture corrections
replace the reserved `window.closed` counter and implement actual portal
introspection. Earlier failures remain recorded under
`notification-integration-v1-ctest.log`.

The source-frozen production engine is recorded under
`notification-refinement-v3`; its exact archive hash is retained in the
qualification state. V4 changes only two native fixture files, starts the
restart-scoped portal setting at startup, and verifies all production inputs
and that archive remain identical. The corrected run passes all seven browser
notification scenarios (nine Qt checks), all 19 transport checks and the
existing notification reset-scope regression (three Qt checks). The real
production QML is embedded, with no source override. Strict workspace Clippy
and the complete repository gate both passed. The terminal
`notification-refinement-v4-status.json` and gate summary record 1,040 Rust
checks passed (two existing ignored) and 544 QML checks passed. These results
cover the notification source freeze; later chooser changes require their own
qualification.

Explicit notification consent after unavailable saved-policy reads,
profile-wide notification attribution when several windows share a Qt profile,
final whole-window review and the other original requirements below remain
open. No installed browser/profile or remote branch was changed.

The chooser investigation reproduced two production defects in all four window
cases: an empty default retained the previous upload in the primary window,
and secondary, private and popup requests belonged to the primary window.
`file-chooser-native-before-v2-tests.log` retains those failures. The replacement
fixture uses pointer double-clicks on real picker rows, checks the actual window
and request owner, and exercises cancellation, successive uploads, competing
requests and navigation. It replaces an earlier test that preselected a file
before an unrelated request and relied on Qt rejecting the empty reset.

The integrated repair creates fresh file and folder dialogs, guards callbacks
from retired instances, routes requests through their owning window and holds
that window's native prompt lease. A competing request is rejected while the
existing selector remains usable. All preparation paths share the same code.
Default filenames are resolved against the chooser folder before conversion to
a local URL, preserving literal percent sequences, delimiters and Unicode.
Ten focused QML checks pass in `file-chooser-integrated-v3-qml.log`. The
`file-chooser-refinement-v1` release build passed, and its compiled primary
browser case passed. The secondary case then exposed a repeatable segmentation
fault during QML engine teardown, so this batch is **not qualified**. Both the
combined run and a separate secondary-only process reproduce it. Core stacks
place the fault in Qt fallback-dialog bindings while a popup window is being
destroyed by the QML garbage collector. Memory availability and the kernel
journal do not indicate resource exhaustion. Qt debug information was requested
but the innermost Qt frame remains unsymbolized; the stack does not by itself
identify a particular QML binding.

The matching Qt 6.11.2 source reparents its fallback implementation to the
hosting window and leaves it there when the public dialog wrapper is destroyed.
It also returns at most one selected file from that fallback. The current
per-request wrapper therefore does not establish per-request backend ownership
or working multi-file selection. The next corrective experiment uses public
`QFileDialog` APIs with explicit ownership, window transience and multiple-file
support. It must prove actual input, bounded cleanup, cancellation, owner loss,
QML engine teardown and native integration before replacement is integrated.
Path-conversion qualification remains pending because the combined native run
ended before that case. Prototype results alone do not establish production
correctness.

The public native/widget replacement is now integrated under the
`native-file-chooser-v2` source freeze. `NativeFileChooser` explicitly owns each
`QFileDialog`, sets its actual host as transient parent, restricts selection to
local files and preserves the host's font/palette when exposed by Qt. Finished
dialogs are destroyed and decisions emitted after the native completion stack
has unwound, so a completion can safely destroy its controller. The existing
GUI bootstrap still exposes `QGuiApplication` while constructing `QApplication`
with owned, byte-preserving argument storage. Downloads, journey/report exports
and userscript manifests use the same backend. Qt Widgets is now an explicit
build dependency.

Eight permanent backend checks pass, including actual pointer selection of
multiple files, literal save names, cancellation, completion that destroys its
controller and eight owner/engine teardown cycles with no retained widgets.
Ten QML chooser integration checks also pass. The browser fixture now covers
navigation, renderer loss and child-owner close across primary, secondary,
private and popup windows. Renderer failure now clears outstanding requests
before offering recovery, and the ordinary status explains the page failure
without engine terminology. The fresh release build and compiled browser cases
are pending; no full-source pass is claimed. Browser-level multi-file/directory
uploads, required-portal transport, chooser rendering and the final combined
gates remain open alongside all seven original goal areas.

A further source audit found that `QFileDialog` saves recent directories in
global Qt preferences. The replacement must isolate that state and prevent
history from crossing regular/private requests before product readiness.
The focused prototype/backend/QML tests wrote `/home/tom/.config/QtProject.conf`;
the side-effect audit records only its metadata, groups, keys and hash. No
before-image was captured. Qt replaces this file atomically, so its inode birth
time cannot establish whether it was new, and the file is preserved rather than
claiming restoration. Subsequent compiled qualification uses a sandboxed XDG
configuration/cache/data directory. That test isolation protects the host; it
does not establish product privacy isolation. The fresh production build may
still be used to qualify chooser ownership and teardown under that sandbox.

The first `native-file-chooser-v2` CMake run is invalid as evidence for the new
backend. Although the browser build refreshed the hashed engine dependency
archive, the top-level archive copied by that harness still had the exact
`file-chooser-refinement-v1` hash. Its widget-driver lookup failures and old Qt
Quick teardown crash therefore came from the previous runtime. The corrected
qualification selects both engine and browser build targets, consumes Cargo's
JSON artifact paths and checks embedded-resource parity before behavioral
checks. The old failed run remains retained. This is a build-harness correction,
not a passing result for the replacement.

The corrected production archive is
`native-file-chooser-v2-corrected-engine.a`, SHA-256
`0fae02af4c55fed4fd28688c003d4b7cee4a1973d41bcb825ab3024816684344`.
Its resource-parity check matches all 102 embedded QML/JavaScript inputs. The
initial widget fixture used `QTest::mouseDClick(QWidget*)` as though it supplied
the full click sequence used by the QWindow overload. The installed SDK shows
that the widget overload sends only a double-click event. The corrected fixture
selects the actual file row, verifies the selected URL and clicks Open. With
that correction, `native-file-chooser-v2-pointer` passes all 16 compiled checks:
11 chooser lifetime cases across the four window types, local URL conversion,
renderer-held-confirmation recovery, resource parity, initialization and cleanup.
This establishes the new backend's single-file flows and removes the repeatable
Qt Quick fallback teardown crash from those production cases.

The privacy investigation independently reproduced a real defect: cancelling
the unmodified widget backend saved a private fixture directory in
`FileDialog/history` (`chooser-history-before-details.log`). The candidate fixes
that failure, preserves a host-preference sentinel, removes its UI preference
directory after normal shutdown, and leaves no private directory in saved state
after a hard kill with both a prior closed chooser and an active one.

That repair is integrated in the `native-file-chooser-v3` freeze. Toolkit UI
preferences have a disposable scope activated after desktop-theme startup and
retained through QApplication teardown. Each chooser forgets its history,
sidebar browsing locations and current directory before Qt saves its state.
Actual request results and per-window folder intent remain available in memory.
Production and test bootstraps use the same GUI factory. Local URL validation
is shared with the browser's existing conversion, including localhost aliases,
without converting paths through display text.

The permanent integrated suites pass ten backend checks, nine privacy checks
and ten QML checks. Browser cases now also exercise real multiple-file selection,
folder cancellation and directory uploads, and record chooser/host renders when
requested. Their expanded source compiles. The selected engine/browser release
build completed successfully. The first complete QML rerun passed 553 checks
and exposed one Guide keyboard fixture failure. The Guide test also fails on
its own because its separate application window is not active; explicitly
activating that fixture before each check passes all 45 Chrome interaction
checks. Native chooser keyboard acceptance/cancellation is covered by the real
widget backend test rather than obsolete Qt Quick fallback internals.

Three test-only files differ from the successful production build freeze;
all production inputs remain byte-identical. The browser matrix now asserts
that acceptance and cancellation return both active-window and page focus
without a test-side activation request. The corrected complete QML rerun and
compiled browser qualification were started separately. The corrected complete
QML suite passes all 554 checks (`native-file-chooser-v3-fixtures-activated-full-qml.log`);
no product focus claim is based on the isolated fixture correction. The native
browser run is still pending. Render inspection, clipboard
and combined gates remain pending. Required-portal transport, configured-font
and theme review, and all original goal areas below remain open.

### Chooser activation and configured appearance follow-up

The stronger browser focus assertion found a product defect as well as the
separate QML fixture issue. In the frozen `native-file-chooser-v3-fixtures`
browser run, cancelling the widget chooser leaves its requesting QQuickWindow
inactive in both primary and secondary windows. Repeated cases fail the same
assertion before the expanded uploads are reached. The redundant failing
matrix was deliberately stopped after those reproductions; its failed status,
logs and explicit stop record are retained. This was not a timeout or a passing
upload qualification.

Rendered evidence also shows that the widget fallback was light even when the
requesting browser was dark. Its owner exposes a QML `Palette` object, so testing
only conversion to the C++ `QPalette` value silently ignored the browser colors.
The correction reads the documented active, inactive and disabled color groups
through public `QQmlProperty`, without private Qt headers. Qt documents these
interfaces in [Palette](https://doc.qt.io/qt-6/qml-qtquick-palette.html) and
[QQmlProperty](https://doc.qt.io/qt-6/qqmlproperty.html).

The `native-file-chooser-v4` source freeze includes these corrections:

- Record whether the widget owned activation before it hides. After destroying
  that widget, request activation only for the same visible owner and only if
  another window has not taken focus. Item focus remains with the existing
  browser composition root. Background cancellation must not activate its owner.
- Transfer the requesting window's palette groups and configured font to the
  widget fallback. Preserve custom disabled and inactive colors.
- Size metadata columns to their contents and give the filename column a
  font-scaled initial width. Wider tables can scroll horizontally; the filename
  column remains resizable. Large text no longer retains cramped small-font
  metadata widths.
- Retint only transparent dark monochrome toolbar icons to the corresponding
  button-text palette, with a distinct disabled state. Colored and opaque
  desktop icons remain intact. This corrects unreadable black navigation arrows
  in the dark fallback without replacing the desktop's file icons.
- Capture browser renders only after pointing the chooser at the disposable
  fixture directory; do not retain personal home-directory listings in new
  qualification images.

The current native backend suite passes 17 checks, including real keyboard
acceptance/cancellation, activation return on either outcome, background
cancellation, four dark/light and 10/20-point appearance cases, and teardown.
All nine privacy checks remain green (`native-file-chooser-v4-icons-tests.log`).
Another six checks pass at scale 2 (`native-file-chooser-v4-dpr2.log`). Populated
dark/light 20-point renders and the scale-2 dark render were inspected: file
selection, text fields and action buttons are readable, with horizontal overflow
available for metadata. The source-selected engine/browser rebuild completed
successfully. Its exact Cargo artifacts are retained in the qualification
record; the engine archive is SHA-256
`c54989574a93ba2d7d3f0f44f7fe0208a55ca1fda2f4bd97bf08e1ff6cf87d48`.

The fresh compiled browser group passes all 16 checks in 45.5 seconds. All 11
chooser scenarios across primary, secondary, private and popup windows exercise
actual single/multiple/directory uploads, file/folder cancellation, active-window
and page focus return, and navigation, renderer loss or owner closure. Resource
parity, local-path conversion and renderer-held-confirmation recovery also pass.
Actual browser chooser/owner renders were inspected; this interaction fixture
intentionally hides browser chrome and is not the final whole-product walkthrough.
The refreshed QML runner, compiled with the changed backend, passes all 554
checks (`native-file-chooser-v4-full-qml-details.log`). All 81 production clipboard
regressions also pass in 131.9 seconds (`native-file-chooser-v4-clipboard.log`).
The qualification runner has terminated successfully, and all 518 frozen
source/data inputs still match. Formatting and whitespace checks pass. These
widget/offscreen checks do not establish
physical Wayland input or required portal transport; final combined gates and
the other original goal areas remain open.

The next ordinary-feedback correction must cover UI action dispatch, the
interactive `action` command, external command starts and registered userscripts
together. The current UI dispatcher overwrites command-owned clipboard pending
notices with internal action IDs, preventing the clipboard completion notice
from being published. External starts also expose operation IDs, while registered
userscripts need their human label rather than merely removing all start
feedback. Stable structured audit/discovery IDs and typed safe errors must be
retained. Portal qualification must also distinguish unsupported remote results
and service failures from a user's cancellation; the current backend collapses
those outcomes into rejection. These are remaining implementation tasks, not
passing claims from this checkpoint.

## Action feedback and operation handoff batch

The compiled `action-feedback-v1-before-details.log` run uses the qualified v4
engine with new browser fixtures. It passes six checks and reproduces two
failures: UI URL copy leaves `Executing browser.url.copy` after the clipboard
write succeeds, and an unavailable session action exposes
`durable-storage-unavailable`. Command URL copy and both selection-copy paths
already reach their completion notice; selection's asynchronous callback
replaces the temporary internal-ID notice. Do not describe all clipboard paths
as functionally broken by this dispatcher defect.

Current source corrections preserve command-owned pending/completion notices
and remove dispatcher notices containing action IDs. Built-in UI dispatch uses
the existing typed `PublicError` boundary, and capability validation supplies
plain guidance while retaining its stable machine reason in diagnostic context.
The same typed validation reaches IPC and switcher routing; structured discovery
reasons and audit IDs remain available. Failed UI availability checks now receive
the matching audit category rather than losing the failure before dispatch.

Configured external commands report a human start notice without an operation
ID. Registered script actions use their validated display label. Selection
preparation keeps that label through the asynchronous handoff, so the eventual
start notice identifies the action the user chose rather than its namespace or
generic extraction machinery.

The handoff review also found that external selection extraction allocated a
new process operation ID, leaving the accepted selection operation pending.
Process launch now accepts the existing operation ID for this internal handoff;
ordinary launches still allocate a fresh ID. Detached commands retain a child
waiter, without tying it to the requesting window, so completed launches can be
reaped while the browser continues running. Rust's
[Child documentation](https://doc.rust-lang.org/std/process/struct.Child.html)
describes the caller's responsibility to collect child exit status.

The public CLI probe independently reproduces the full tracking defect on the
frozen v4 browser (`action-feedback-v1-operation-before-focused.log`): a real
selection command produces three records—an accepted request, a pending
selection, and a completed child. The earlier probe lacked a retained selection
source and is kept as a fixture limitation, not successful-launch evidence.
The corrected fixture explicitly enters Insert mode in its owned page before
extracting its stable selected text. IPC now adopts an asynchronous provider's
operation ID in the reply, command context and action audit, and preserves its
already-recorded state instead of overwriting it with accepted. This preserves
provider identity without giving separate commands in a chain the same new
process ID.

New browser fixtures cover actual native clipboard completion through UI and
command entry, registered URL/selection scripts with protocol-input capture,
and tracked/detached external processes through both entry points. A separate
public CLI/IPC probe is being prepared to verify that selection preparation and
launch produce one terminal operation, and that completed children are reaped.
The initial strict workspace check found one unused import after removal of an
error rewrap. After its removal, all-target workspace Clippy passes. The selected
engine/browser release build also passes. Its first native action run passes
15 checks and exposes five failures: four `send` command cases reach an
unsupported-command error because interactive dispatch never connected that
registered command to its existing adapter, and one selection fixture races an
older non-null metadata capture. Interactive dispatch now handles `send` with
the same validation and provider used elsewhere. The fixture explicitly waits
for a fresh selection capture; it no longer treats an old non-null lease as
proof that its newly selected text is ready.

Manual qualification against that frozen first corrective build passes the
public configured-target probe for tracked and detached selection commands:
both retain one terminal operation and their children are reaped
(`action-feedback-v1-operation-corrective.log`). The existing public IPC probe
passes, including rejected-transfer completion and command routing. All 81
clipboard regressions pass in 132.7 seconds. The interactive-dispatch correction
and revised fixture still need their new build and complete native action run;
the next all-target Clippy passes. Revised fixture objects also compile. Command
rows now submit through the real browser command line, including focus transfer
from the selected page, rather than only calling the backend command invokable.
The `action-feedback-v2` source freeze contains 520 inputs and its selected
engine/browser build is running. Its complete corrective action run is still
pending. The public probe and native regression
group are included in CI. The earlier v4 chooser/QML checkpoint remains valid
for its frozen inputs; none of these narrower checks claim final whole-product
qualification.

The selection handoff audit must also cover generic `spawn` selection and rapid
replacement of pending consumers. Generic spawn currently has no operation ID
in its pending selection result and launches a new process ID after extraction;
the configured-target probe does not qualify that separate path. These remain
part of the original operation/input consistency work.

Only the disposable `target/debug/incremental` cache was cleared to recover
build space (about 44 GB). No compiler using this workspace was active at that
time. Source, frozen artifacts, binaries, logs, `tools/`, installed browser data
and normal profiles were preserved. Subsequent compilation disables incremental
caching for this run. Final combined gates and all original outstanding goal
areas remain required.

## Selection actions and command completion follow-up

Actual command-line submission exposed a defect that backend-only action
checks missed. The native action group on `action-feedback-v2` passed 16 checks
and failed four selection-command rows. URL actions, registered action labels,
and external UI launches passed. Selection commands were accepted, but the
successful QML submit path then called Escape, which cancelled the newly
accepted work and restored its earlier “Preparing selection” notice. Both the
primary and secondary submit paths used this cancellation handler.

Successful submission now completes command entry through a separate bridge
method. This resets command mode and completion presentation without revoking
accepted asynchronous work. Explicit Escape retains its cancellation behavior.
The primary existing-context-window shortcut also uses the completion method,
because that shortcut consumes submission before ordinary Rust dispatch. The
new native group uses actual interactive submission, checks clipboard contents
and meaningful completion notices, and includes a secondary-window selection
copy case.

The public IPC follow-up independently reproduced two lifecycle defects using
owned loopback pages and disposable profiles:

- Generic `spawn` with a selection placeholder created a completed child
  operation while leaving the original command operation accepted indefinitely
  (`selection-lifecycle-before-identity-v2.log`). The extraction request and
  spawned process now retain one operation identity, and extraction failures
  finish that operation.
- A renderer reply could revive a cancelled configured-selection operation
  (`selection-lifecycle-before-cancel-v3.log`). This negative-only run briefly
  overlapped compilation; it establishes an observed state transition, not
  passing timing qualification. Cancellation, replacement, navigation and
  timeout now retire the selection request and all its consumers together.
  Delivery checks the current request token before routing any reply, so a
  retired callback cannot launch work or replace newer feedback.

Ordinary selection failures now explain how to recover rather than showing
selection-source codes. Explicit operation diagnostics retain extraction
reasons. Queued userscript results also check cancellation before applying a
returned page action. This last guard still needs an actual queued-result
regression; source inspection alone does not establish its runtime behavior.

The held-reply fixture was then refined to stop its startup selection mutation
and establish readiness through a successful public selection action. Against
the frozen earlier binary, all three cases reproduced their intended defects
without concurrent compilation (`selection-lifecycle-stable-before.json`).
Only that Python fixture changed after the successful production build: all
other 521 frozen inputs, including all production code and resources, matched.
The original build manifest and the one-file qualification delta are retained.

A separate owned D-Bus service and actual Qt 6.11.2 xdgdesktopportal theme also
reproduced the outstanding chooser cancellation problem. The toolkit issued
OpenFile with multiple-selection enabled, and the browser chooser emitted one
rejection when closed, but the service observed zero Request.Close calls
(`file-chooser-portal-probe-before.log`). This is private-bus/offscreen transport
evidence; the empty offscreen parent identifier does not prove Wayland parent
behavior. Portal cancellation and precise unsupported-result feedback still
need implementation and qualification after this frozen selection batch.

Four Rust regressions cover consumer cancellation, unrelated-operation
isolation, preservation of terminal results and recovery wording. The first
strict workspace Clippy run and native fixture object compilation passed.
Review then added the shared command-completion bridge and refined selection
copy. The final source is frozen as `selection-lifecycle-v2` (522 inputs), with
492 engine Rust tests passing (one existing ignored), including all four new
selection regressions. The selected release engine/browser build passed. Its
exact native executable passes 21 action checks, including successful primary
and secondary interactive selection commands. The public CLI probe passes both
tracked/detached launches and child reaping; the public lifecycle probe passes
identity, cancellation and replacement; the existing public IPC probe also
passes. The older CLI fixture needed the same acknowledged, quiescent selection
setup as the lifecycle fixture: its periodic DOM selection changes correctly
triggered changed-selection rejection. Only that Python fixture changed; all
production/native fixture/resource bytes and tested artifact hashes were
preserved. The source manifests record both fixture-only deltas.

The clipboard group then passed 79 cases and rejected two notices: generic
selection recovery had hidden why password fields and unsupported controls
cannot supply plain text. The V3 mapping restores those specific explanations,
keeps “shadow” implementation terminology out of ordinary notices, maps the
extraction-stage changed-selection reason, and explains an oversized selection.
The tests retain their existing blocked-control expectations. Extended unit
assertions cover the explicit password/control explanations.

V3 has a fresh 522-input freeze and a successful selected release build. Its
exact native executable passes 26 checks, including the real password and
unsupported-control selection cases alongside the complete action group.
Tracked/detached CLI launches, lifecycle identity/cancel/replace and the existing
public IPC probe also pass on this V3 binary. All 81 clipboard cases now pass
(131.82 seconds), including both blocked-control cases that rejected V2's
notices. Repository/QML/Rust and strict Clippy checks pass on the same frozen manifest. The repository gate
includes 554 QML checks and 1045 passing Rust tests (2 existing ignored).
`selection-lifecycle-v3-status.json` records the terminal passing checkpoint.
The new notices now have actual native evidence; V2 passes alone were not used
to prove the changed V3 messages. Final combined walkthrough and the broader
outstanding goal areas remain required. Selection
copy/search operation completion and other engine handoffs also require the
separate IPC-003 audit; process-launch checks do not prove those consumers.

## Parented file chooser integration follow-up (17 October)

The isolated Qt 6.11.2 platform-theme probe reproduced a cancellation defect:
the actual FileChooser portal received `OpenFile`, but closing the browser's
controller emitted local rejection without sending `Request.Close`. The owned
widget checks did not establish that transport behavior. The supported public
window-parenting API was subsequently verified through KWindowSystem 6.28's
`KWaylandExtras::exportToplevel()` with an actual Wayland window and process.
The compositor fixture exports real `wl_surface`/`xdg_toplevel` resources and
does not use private Qt APIs or interact with the user's desktop.

Production file selection now has an application-owned Qt D-Bus request actor
and a per-request controller. The actor uses its own bus connection, pins the
portal service owner, subscribes before sending the request, and closes the
request on cancellation, including when its initial reply arrives late. Initial
transport and cleanup have bounded deadlines; users have no arbitrary time
limit to choose a file. Installed portal services can activate on demand through
D-Bus. The parent export is retained for its native window's lifetime so one
chooser cannot invalidate another chooser's shared handle.

The backend snapshots each request's intent and explicitly owns its widget
fallback. Auto fallback is restricted to failures discovered before sending
the portal picker request. Required mode cannot silently bypass a missing
service or unsupported parent. After a request has been sent, an uncertain
response does not open a second picker. Unsupported URI schemes, malformed
results, empty acceptance, and transport failure are separate from user
cancellation, and no partial selection reaches the webpage. Literal filenames
and symlink-sensitive dot segments retain their meaning.

The shared QML surfaces preserve export previews after failure, report useful
guidance to the upload's actual owner, and keep failed destination choices
retryable as interrupted downloads. The authoritative download owner retains
that cause through Qt's subsequent terminal callback. The secondary-window
download picker, previously still a separate QML `FileDialog`, now uses the
same owned controller. Primary, secondary, private and popup upload failure
paths have production-fixture checks added for qualification.

Build and packaging metadata require KWindowSystem >= 6.28 and describe its
license. Ubuntu CI builds pinned, checksum-verified source releases into a
disposable prefix and installs the exact Qt Wayland Compositor SDK component.
The isolated dependency build completed locally; no system library or desktop
configuration was installed or changed. The fixture additionally checks the
loaded library and platform plugin paths against that prefix.

The first integrated run passed 38 portal checks but an existing privacy
assertion caught an extra trailing directory slash. The implementation was
corrected without changing that assertion. The second frozen run passed 41
portal checks and all 26 widget/privacy checks. Its ordinary QML cases passed
556 checks; a newly added registry test could not load native profile types in
the lightweight runner, so it was moved into the complete browser fixture
instead of adding fake native implementations. The frozen V3 source/build
qualification is recorded in `portal-integration-v3-source.json`,
`portal-integration-v3-artifacts.jsonl` and the corresponding runner/status
files under `target/product-polish/`; it is not yet a passed checkpoint.

The V3 release build completed successfully, and the exact release artifacts
passed all 41 portal and 26 widget/privacy checks. The complete browser suite
completed 182 checks before an existing diagnostics-export fixture blocked on
the widget chooser's overwrite confirmation. The fixture now drives that
owned confirmation with a bounded timer, waits for deferred export completion,
activates the picker before acceptance, and observes owner activation before
Escape. The existing-file bytes, refusal-to-overwrite and reviewed-report
assertions remain intact. The corrected focused diagnostics test passes in
2.1 seconds. Production source and frozen release binaries are unchanged;
only the native fixture differs, as recorded by the V3a/V3b source manifests.
The V3b runner repeated the complete browser suite on a disposable private
bus: all 189 checks pass in 474.9 seconds. Its repository gate then found one
obsolete source-contract assertion for the removed separate secondary download
dialog. V3c updates that test to the shared chooser and retained download-owner
completion path. Production inputs and release binaries remain unchanged.
The V3c repository gate passes all 1,045 Rust tests (two existing ignored), all
556 QML checks, profile-lifetime and packaging/license checks. Strict workspace
Clippy with all targets and `-D warnings` also passes. The manifests and
`portal-integration-v3c-status.json` preserve that final checkpoint and the
fixture-only deltas.

A separate source audit found recovery work still to verify: the QML required
preflight can reuse a negative capability result instead of refreshing it on
retry, while the new native controller already performs fresh per-request
checks. Immediate opening feedback and an actual Qt download failure/Retry
journey also need follow-up. These findings are recorded in
`file-chooser-recovery-followup-audit.json`, without changing the frozen inputs.

This evidence does not establish real desktop portal UI focus, physical input,
or capture consent. Those, the remaining original goal areas, final combined
gates and whole-window walkthrough still need qualification.

### File chooser recovery and opening follow-up

Actual browser requests on the owned Wayland display reproduced two more
failures. After a portal service returned, the QML preflight still rejected
uploads using its earlier unavailable result. This was reproduced in primary
and secondary windows with trusted page clicks. A separate held-opening case
proved that Escape reached the page without cancelling the outstanding picker
request (`portal-opening-observed-baseline.log`).

File requests now reach the native controller's fresh, required-mode-aware
discovery without a second cached availability gate. Uploads report opening
immediately. Every shared chooser intent also has a cancellable opening surface
in the requesting window's modal input scope and appearance. It owns input
immediately but delays painting by 200 ms to avoid flashing during fast opens.
When the native picker opens, the preparation surface yields without restoring
page focus into the picker. Cancellation closes the outstanding portal request
and releases the browser's prompt lease.

The development browser run passes all 12 behavior cases: service recovery,
Escape cancellation and pointer cancellation in primary, secondary, private
and popup windows (14 QtTest passes including setup/cleanup). It verifies the
actual exported parent window identity and the page's selected Unicode filename.
The compact opening surface was rendered and inspected. This development check
loads source QML through a test-only public URL interceptor against the frozen
V3 native archive; it is not a compiled-release qualification.

The normal application CMake harness and CI now include `portal_browser`, using
the Cargo-built engine and compiled QML resources. The frozen V4 engine and
browser release build passed. Its first browser run passed 11 behavior cases
but the cold-start fixture queried the page before its main frame existed.
V4a adds that missing frame-readiness observation without changing the compiled
production source: all 12 behavior cases now pass against the frozen release
(14 QtTest passes including setup/cleanup), alongside the 41 standalone portal
checks. `portal-recovery-v4a-source.json`, its test-only delta and status preserve
the evidence. The stopped V4 driver did not run the remaining broad gates.
An actual Qt download destination failure followed by Retry and Escape routing
during non-upload chooser preparation still need separate end-to-end checks.
The remaining original goal areas below stay open.

### Modal input and Downloads follow-up

The actual Qt download journey exposed a separate cancellation gap. During a
held SaveFile opening, Escape was classified twice by the browser key router
and did not cancel the request (`portal-download-retry-before.log`). Browser
key routing now yields whenever its own window has an active modal input
surface, covering preparation surfaces that do not hold an upload prompt lease.
The source runtime check confirms zero browser key classifications and complete
request cancellation in primary, secondary, private and popup download owners.

The same journey retained an interrupted download correctly but exposed Retry
before its metadata refresh finished. The action returned a generic failure;
once metadata settled, the original URL and real saved payload were correct.
Downloads now shows loading/update/failure states, offers Refresh, and enables
metadata-dependent file/history controls when the read is ready. Completing a
read updates the visible panel without waiting for its metrics timer.

The compiled V4 baseline also proved that rebuilding the download model lost
the expanded row after a presented frame (`download-manager-frame-before.log`).
Refreshes now reconcile stable record IDs in place. The complete development
journey uses the real Details and Retry buttons, observes a rendered refresh,
checks readiness, receives a second actual SaveFile request and verifies the
final file bytes. All 17 behavior cases pass with source QML (19 QtTest passes
including setup/cleanup), recorded by `download-manager-source-after.log`.
The compact renders also exposed a moving row when the update notice appeared
and disabled controls without a distinct visual state. The notice now retains
its layout space, is excluded from accessibility when hidden, and disabled
metadata-dependent buttons are visibly dimmed. Both ready and updating renders
were inspected after another passing 17-case development run
(`download-manager-source-final.log`). Frozen V5 qualification passed: the
selected engine/browser release build, all 17 browser journey cases (19 QtTest
passes including setup/cleanup), 41 standalone portal checks, 26 widget/privacy
checks and all 189 production browser checks passed. The repository gate passed
1,045 Rust tests (two existing ignored), all 556 QML tests, profile lifetime,
packaging and license checks. Strict workspace Clippy with all targets and
`-D warnings` passed. `portal-polish-v5-source.json` and
`portal-polish-v5-status.json` retain source/artifact hashes and each gate log.

One compiled image named as ready still displayed the update notice. The
state-aware observation probe subsequently passed after two presented ready
frames: pending was false, Retry enabled and caption opacity zero before and
after grabbing; the inspected image had no update caption. The updating image
and its facts also matched. This resolved the capture ambiguity without a
production change. State-aware capture is now in the working test harness.

### Download record identity across restarts

The normal-mode two-process experiment did reproduce a separate correctness
failure. Both actual failed-picker/Retry journeys and saved-byte checks passed,
but the second process reused native request IDs `1` and `2` and replaced both
first-process history rows. The tagged source URLs and database backup are in
`download-restart-probe/run-d95ea3bc73f44e70b2cdd028a50a9196/`. The first audit
attempt hit Chromium's different downloads-table schema and made no retention
comparison; the corrected reader selects the browser metadata schema explicitly.

The working fix allocates an application UUID for each admitted request and
retains it in the native request's owner entry. The same identity reaches
progress, destination selection, completion and teardown. Existing stored IDs
remain unchanged and readable; no database migration is required. A native
ownership regression covers duplicate notification, two objects with the same
native integer, cancellation and an ignored late completion. The normal harness
also has `portal_download_history`: two browser processes share one marked
disposable profile, with separate source tags and saved files; it checks all four
records, exact old-row retention and both payloads. This test is included in CI.

These additional changes are frozen for V6 build/native/history/browser,
repository and Clippy verification (`download-identity-v6-source.json` and
`download-identity-v6-status.json`). Their results are still unproven. The V5
qualification above applies to its preserved source and artifacts, and the full
original polish goal remains open.

Ordinary typed runtime/IPC error preservation, private download-history wording,
active-control behavior during sustained metadata updates, and durable download
record identity/display names still need their separate audits. These findings
do not narrow the original remaining scope below.

## Context deletion provenance investigation (17 October)

An isolated normal-mode public CLI/IPC journey confirms another real data-loss
path. After entering and saving a context defined in contexts.toml,
`context-delete work --confirm` removes its generated record and reports
acceptance. The authored definition is unchanged. A subsequent configuration
reload creates the context again with a different UUID and empty saved
membership. The successful reproduction is retained in
`target/product-polish/context-delete-audit/c3094314819f4dd79d6fb254e3d541a6/`.
Two earlier fixture attempts failed before startup and provide no deletion
evidence; the corrected fixture uses a short disposable Unix-socket base.

The repair was prepared in the isolated checkout
`target/product-polish/context-delete-worktree` while V6 remained frozen, then
integrated after that driver terminated. It
checks the freshly loaded catalog's existing configured-name provenance under
the registry lock, rejects direct deletion of contexts defined in the source
file, and explains the edit/reload path. Command-created contexts remain
directly deletable. No source document, route, identity or saved membership is
removed on refusal. The error remains typed through CLI/IPC so users receive
the same guidance instead of a generic engine failure.

The storage/core run passes 255 tests, including refusal from an outdated
registry owner and preservation across reopen. The actual two-process CLI
regression fails against the V6 browser because it accepts the deletion; an
initial fixture attempt used an unsupported output flag and is excluded from
that conclusion. The corrected regression uses ordinary CLI error output.

Command-bar, reload/reopen and two-process public CLI/IPC regressions are now
integrated. Context help explains these operations separately from profile
deletion; hints, target search, Grid and tab lifecycle articles now explain
actual controls and limits without internal Rust/lease/qualification wording.
The shared command reference is regenerated. Eight new whole-application Guide
walkthrough rows exercise keyboard selection, reading, scrolling and dismissal
at normal and larger fonts, with optional actual-window captures. These native
checks and the positive public CLI run still require compiled verification in
`consent-context-polish-v1`. None of this establishes completion of the wider goal.

## Explicit notification consent after a failed saved-policy lookup

An actual normal-mode Qt journey confirms this remaining permission defect.
The healthy-policy control clicks the real page button and Allow once button,
receives `granted`, submits a real private-bus Notify call, and receives one page
show event. With an invalid stored decision for that exact origin, the browser
warns that saved permissions could not be checked and offers Allow once. Qt
still grants access after the same real button click, but the presentation
controller starts another saved-policy wait and cancels the notification.
There is no desktop delivery or page show event. Facts, prompt PNGs and owned
SQLite snapshots are retained in
`target/product-polish/notification-once-probe/run-v1-renders/`; the preserved
V6 engine is recorded by hash. The control passes and the defect case fails
at shown=0 versus expected=1, rather than at setup or permission admission.

The repair removes that second permission wait. Only Qt-authorized notifications
enter this controller; Ferric profiles use AskEveryTime and the application
permission-request handler owns the decision. Definite Deny, revocation,
disabled notifications, owner lifetime, and required service checks remain in
force. Portal waiting retains its bounded, non-renewing deadline and queue cap,
and now cancels promptly if notifications are disabled or a Deny arrives.
No shadow grant store, persistent choice or alternate transport is added.
The detailed authority contract and pinned Qt reference are recorded in ADR-0004.

The isolated component run passes 34 checks, covering delivery with a pending
saved lookup, known/late denial, required service handling, expiry, ownership,
private-session tags, duplicate delivery and close/click behavior. The repair is
now integrated. A standard native consent suite is included in CI and covers
healthy policy, the reproduced invalid row, saved Deny, and denial before the
service acknowledges. Production compilation and positive native qualification
are still pending; this evidence does not close the shared-profile attribution
or wider final polish audit.

The actual prompt captures also exposed a partly clipped final permission
choice at 900×700 and 12-point text. A new component check reproduced that
layout failure with the initial measured-height change (107 pass, one failure).
The shared fit calculation now accounts for the action area's reserved height
fraction, and the permission prompt uses that calculation. All 108 prompt
checks pass, including both ordinary and unavailable-policy four-choice states.
The native consent suite additionally checks all four choices against their
actual clipping ancestors before accepting input.

The combined `consent-context-polish-v1` run freezes this complete source and
builds production artifacts before the consent, context/Guide and CLI journeys.
It then runs portal/history, the broad browser suite, the full repository gate
and fresh strict Clippy. Its compiled results and final render inspection are
still pending. The earlier V2 repository run passed 556 QML checks but stopped
on the application test's old configured-context deletion setup; that scenario
now uses a manual context, and all five application consistency checks pass.

### Compiled consent follow-up

The frozen `consent-context-polish-v1` release now passes the four actual
normal-mode consent journeys (six Qt checks including setup/cleanup). A real
Allow once with an invalid saved-policy row receives one desktop delivery and
one page `show` event. The healthy control also receives one. Stored Deny
receives neither, while denial before acknowledgement closes desktop ID 77
before owner teardown and never sends a page `show` event. Read-only snapshots
confirm the once-only choice creates no persistent permission row and preserves
the invalid row rather than silently rewriting it.

The three actual 900×700 permission captures have been inspected: all four
choices, explanatory text and keyboard guidance fit without clipping. The
unavailable-policy state retains its warning and disables the persistent Allow
choice. The wider native notification regression also passes (nine Qt checks,
including setup/cleanup). The combined run then stopped in the context/Guide
phase (28 passed, four failed); portal/history, repository and fresh strict lint
phases were not reached. This is not a full checkpoint or physical-desktop
qualification.

One failure was a real copy inconsistency: an explicit context-aware reload
said “Configuration reloaded” while recovery said “Settings reloaded.” Both
paths now use “Settings reloaded.” The three Guide bounds failures measured the
preceding layout before the first reader frame. A separate frozen-release
measurement retains the original window-bounds assertion after presentation:
all eight articles pass, along with actual Page Down and two-stage Escape
behavior (ten Qt checks including setup/cleanup). Before/after rectangles and all
eight captures are preserved under `guide-frame-probe/`; no product layout was
changed to make that test pass.

Visual review additionally found target search using its entire implementation
description as its heading. All five remaining built-in title fallbacks now
have short authored titles, and hint/search/Grid summaries explain what the
user can do without activation/transport terminology. Generated references are
refreshed and all 154 core checks pass. The newly authored copy and reload
wording still need fresh embedded production qualification.

### Switcher query and source-readiness investigation

Two normal-mode native journeys against unchanged frozen Switcher sources
reproduce genuine failures. Holding a real metadata producer makes an absent
bookmark search show “No matching items” while its read is pending; releasing
the producer makes the actual bookmark appear. Separately, editing “Alpha” to
“Beta” and delivering Enter before the debounce opens Alpha and closes the
Switcher. The probe records the exact URLs, latest query, rows at Enter and
actual-window capture (`switcher-pending-probe/run-v1-*`).

The current repair invalidates the preceding query immediately, presents
“Searching…” during replacement, and guards both default and explicit action
dispatch against the current query, scope and batch generation. A native CI
journey and component input regression now cover the same-turn edit/Enter and
subsequent confirmation of the new result. Source-QML native and component
verification passes 16 component checks. A source-QML native observation
confirms that same-turn Enter leaves Gamma active and keeps the Switcher open,
but its fresh-Beta assertion exposed another real backend defect rather than a
debounce timer failure: once the optional index is ready, the “Tabs” scope also
includes history. The actual snapshot contains a history row ahead of the live
tab at the same search rank. Snapshot fallback already respected the scope.

Indexed library candidates now filter by the requested scope before ranking.
The exact production index module passes two tests with its real storage
dependency; the new case covers every library scope and excludes those rows
from all non-library scopes. Published query/scope identity also keeps usable
same-input rows available while background metadata batches are replaced,
without allowing an old query to become current again.

The `switcher-scope-polish-v1` release completed its actual-window query
journey (three Qt checks), consent/native notifications and the full
context/Guide suite (33 checks). It stopped at the public CLI fixture, whose
expectation omitted the required Quit confirmation; later gates were not reached. No full green checkpoint or
installation-readiness claim follows from the component or isolated-module
checks. The failed source observations and exact captured kinds/ranking are
retained under the Switcher probe prefixes; they are not passing qualifications.

The independent library-loading repair is now integrated and awaiting fresh
embedded qualification, as described in the follow-up below. It uses the
relevant source's actual read/result state, distinguishes pending and unavailable
from a concluded empty search, preserves usable current-query results, and obeys
private/retired owner boundaries. A library index build alone does not imply
unavailability: the query already supports its immutable snapshot fallback.
Shared `library_pending` also includes unrelated journey reads and is not a
sufficient readiness fact. Selection continuity during source replacement and
captured-owner retirement remain part of this Switcher work.


### Switcher ownership and native-page close follow-up

An actual normal-mode, non-intercepted release fixture reproduces a private
secondary tab appearing in the ordinary primary window's Switcher. Its regular
child control passes; private exclusion fails (three Qt checks pass, one
fails). Front-end candidates now capture the invoking UI, window, native
profile object and window token, include eligible popups, reject retired
owners without falling back to the primary window, and mount the surface in
the invoking host with that host's font and input scope. Ordinary searches
exclude transient windows; private searches use only the same native profile
object. Component ownership checks pass, and a source-QML native observation
passes three checks at 20pt in a 640×480 private window, including cross-window
Escape isolation and focus restoration. This observation used a direct
front-end invocation because the frozen backend still rejected secondary
Switcher commands. The integrated backend restriction is removed, and a new
standard native suite exercises the actual command path; fresh embedded
qualification is still required.

Correcting the public CLI fixture to choose its own visible Quit button with
an offscreen-only native Qt input plugin exposed a separate product defect:
`rb://blank` entered a web-script check that never called back, then displayed
“Page state check timed out; choose Close anyway.” The plugin records the
single authorized native click and subsequent state; it does not automatically
accept later prompts or change the production quit contract.

A shared close policy now classifies browser-owned native pages directly and
checks every tab in secondary windows, including background forms. An open
start-page favourite editor still requires confirmation. The focused QML
checks pass (11 close-policy checks, nine ownership checks and 21 start-page
checks). Both new native suites compiled in a preflight using the preceding
archive; this establishes compilation only. The
`switcher-owner-close-v1` production run passed all seven Switcher checks,
including actual private-window command invocation, transient exclusion and
retired-owner refusal. Its actual 20pt private-window capture was inspected.
All six clean close journeys passed; the two dirty-form cases stopped during
fixture setup, so the combined run is terminal exit 1 and later gates were not
reached. No full green checkpoint, installation-readiness claim or
physical-desktop qualification follows from the preceding component and
source-QML observations. The full original seven-area goal remains open.


### Saved-source readiness, stable results and secondary navigation

The form fixture now waits for the actual document and confirms its edit
through a page callback. Against the unchanged embedded owner-close V1 engine,
the primary dirty-background journey passes. The secondary journey then
exposes a real product defect: interactive `tab-open` consumes its navigation
before the scheduled tab-view materialization, silently leaving `about:blank`.
The secondary navigation branch now synchronizes the reducer-owned tab model
before resolving the target view and reports a missing view instead of silently
losing the request. A public QML URL-interceptor observation with that repair
and the unchanged native backend passes all 16 checks: twelve clean
current/new-tab × blank/start/web × primary/secondary journeys and both dirty
background cases. Source hashes and the intercepted resource list are retained
under `secondary-navigation-order-v1/`. This is source-QML evidence, not fresh
embedded production qualification.

Saved library/session read admission now has explicit unrequested, queued,
pending, ready and failed states. The existing coordinator retains scheduling;
at most one not-yet-admitted intent per catalog is retried when its bounded
queue frees. Switcher queries request only the sources needed by their scope.
A failed read waits for explicit Refresh rather than retrying on each keystroke.
Private/temporary catalogs and unavailable profiles have truthful notices,
while optional index construction does not hide the usable snapshot. Qt uses
typed readiness properties. Public IPC availability is computed for the current
response and never stored in a cached row projection. Technical detail is
bounded and displayed as selectable plain text in a separate details dialog;
Escape restores the disclosure control. Actual 390×480, 20pt captures were
inspected; the separate reader gives long errors usable space.

A source component observation first reproduced selection reset and lost
action focus after a result replacement (three checks pass, two fail). The
integrated renderer reconciles stable result identities and retains unchanged
action delegates. Pressed actions retain the owner/ID/generation captured at
press time; a changed generation cannot inherit a held mouse or keyboard
activation. Source batching keeps rows belonging to unqueried live owners,
removes them when that owner has been replaced, prefers the invoking window's
commands, deduplicates catalogs, and applies one combined result limit.
Local identity keys and field comparisons do not serialize presentation rows.
The final candidate component run passes 40 checks, including five compact
readiness states, details/Escape/focus, stable selection and held-generation
input, and combined-source retention/dedup/limits. The pure Rust source model
passes seven checks.

These repairs and two real producer/failure/retry native journeys are now in
the main worktree. Session-list admission moved into the existing session
adapter to keep handwritten Rust modules below 1,000 lines. Formatting and
whitespace checks pass. The `switcher-readiness-close-v1` source freeze has built the actual production
engine and browser. Both new native suites pass (16 close and nine Switcher
checks), including the real pending producer and explicit read-failure recovery.
The current-query regression, consent/native notifications, full context/Guide,
public context CLI and portal/history phases pass. The wider production browser
suite passes all 190 checks, and the repository gate's full QML/WebEngine suite
passes all 607 checks. The repository gate then failed on four source assertions
that expected the earlier primary-window getter, inline result implementation
and notification replacement variable. The native behavior checks above pass;
the source checks are being updated alongside real invoking-owner limit and
held-Enter regressions. Strict lint was not reached. This is not a completed
composite checkpoint. No current complete checkpoint or installation
readiness is claimed. The original seven-area goal remains open, including
remaining IPC/error boundaries, download/privacy feedback, notification
attribution/canonicalization, popup ancestry, selection/userscript completion
and final whole-window review.


### External Switcher authority follow-up

A separate real public-IPC observation reproduces a privacy bypass in the
unchanged owner-close V1 browser: the ordinary private query is denied with
`ipc.private_queries=false`, but adding a matching `private_scope` returns
actual private tab rows. The ordinary tab control and accepted private-window
command are recorded alongside the response. The owned process was terminated
and its temporary profile removed. Two earlier probe attempts had bootstrap/
canonical-error control mistakes and do not establish a product failure; the
corrected negative evidence is `switcher-private-ipc-probe-v3/status.json`.

A separate candidate now selects native versus external authority in Rust
rather than inferring authority from client fields. It preserves explicit
external opt-in and requires an exact live scope for native private searches.
It also preserves typed denied/stale/invalid/configuration errors, separates
safe native feedback from bounded diagnostic detail, and retains the real
recovery action for unavailable sources instead of always suggesting Refresh.
Three pure authorization checks and nine source-presentation checks pass. The
candidate includes a real public-IPC regression for disabled and enabled
settings and a CI step. After the preceding qualification stopped, the candidate
was integrated with base-file hash checks. Its fresh full engine Rust preflight
passes 503 tests with zero failures and one existing ignored test. Formatting
and whitespace checks pass. `switcher-query-boundaries-v1` is now building a
frozen production source snapshot. Its fresh real public-IPC regression passes
both private-query settings with ordinary-tab and private-owner controls:
disabled access returns `E_DENIED` for no, matching and stale supplied scopes;
explicit opt-in returns the actual private row for no/matching scope and
`E_STALE_TARGET` for the stale scope. Malformed input returns
`E_INVALID_ARGUMENT` in both settings. Owned process groups and temporary
profiles were cleaned up. Native Switcher checks pass all 11 cases, including
private/ephemeral isolation, native private invocation, held-Enter confirmation,
invoking-window combined limits and the saved-source producer/retry cases.
All 16 close-dialog checks and the current-query regression also pass. The
broader composite now also passes all 190 browser checks, all 609 QML/WebEngine
checks, native clipboard/file chooser/action/notification transport groups,
context/Guide, public context CLI, public IPC/action/selection lifecycle/window
routing and portal/history checks. Workspace Rust tests pass 1,058 cases with
zero failures and two existing ignored cases; strict Clippy and desktop-entry
validation pass. All 21 phases of `switcher-query-boundaries-v1` are terminal
passed. This is a complete checkpoint for that frozen source, not completion of
the original seven-area polish goal.

The actual native held-Enter observation reproduces unintended navigation:
holding Return while a new query is pending activates its result when the
refresh finishes. `switcher-held-enter-probe-v1` records the failed observation.
With the same native observer and backend, a source-QML guard that ignores
auto-repeat passes all three checks in `switcher-held-enter-probe-v2`: the page
and search remain open until a release and fresh Return press. The guard and
native regression are now integrated. A native combined-limit regression also
checks that the invoking child window's temporary result limit constrains the
combined results while the primary window retains its own setting. These new
main-worktree checks now pass fresh compiled qualification in
`switcher-query-boundaries-v1`. The combined-limit case also emitted a Qt
`DelegateModel::cancel` index warning while updating the list. The passing count
and owner checks do not establish that warning is harmless; retain it for the
remaining list-lifecycle review.


### Download ownership follow-up

The read-only source audit during production qualification confirms a mismatch
that still needs a real transfer regression: private/temporary Switcher guidance
recommends `:downloads`, but secondary command admission blocks that command.
The root manager reads only the primary saved library, while live control policy
also requires durable, current saved metadata. Native transfer ownership already
retains the allocating UI, host and application UUID; the manager does not yet
use that authority consistently. This leaves private transfers and transfers
whose history is busy within the original unfinished scope.

The V1/V2 native observers reproduce private-child manager refusal after an
ordinary-primary positive control. Their directory setup prevented a transfer,
so they provide no progress/cancellation evidence. V3 seeds the typed custom
directory in matching startup JSON/TOML and asserts the owned destination before
starting a held local HTTP response. It reproduces actual Qt progress of 262,144
bytes while the visible row reports zero, and a visible Cancel action that
reports success while the accepted native request continues.

The source audit additionally finds that visible progress reads saved byte
counts while live received bytes feed only the speed sample. Accepted downloads
also reuse the one-shot request resolver for later cancellation; its duplicate
guard rejects the already-decided object. A separate source-only candidate
separates prompt decisions from captured-profile transport controls and reads
live progress. The same native observer and backend pass four checks with this
repair. The refined ownership observer passes nine checks, including actual
pause/resume, foreign-profile refusal, same-profile peers, allocator retirement
with network closure, and duplicate acceptance refusal followed by working
cancellation. Exact source hashes were checked before integrating six repaired
QML files. All 609 source QML/WebEngine checks then pass. The durable native
Downloads fixture compiles and is registered in CI; private-window manager
access remains required and unfixed. Fresh embedded production qualification of
the integrated source is still pending. The new session catalog prototype passes seven pure
checks for owned mutation, saved-history overlay, private snapshot exclusion,
file-finalization publication, bounded active retention and stale write
acknowledgements. It is not integrated. The full repair must use validated
application profile-session lifetime, atomic revision-aware history persistence
and per-window presentation. Private completed records, live control during
metadata backpressure and the whole transfer lifecycle remain required.

The native setup also exposes a runtime setting defect: startup accepts the
canonical tagged custom download directory, but runtime `set` rejects that
table and a scalar path does not match the configuration type. The editor needs
narrow, transactional support with explicit path validation. No directory
setting repair is claimed yet.


### Shared Downloads session and window integration

The shared profile-session resource now owns both permission state and an owned
download catalog. This preserves the validated inheritance, identity and lifetime
rules instead of introducing a second registry keyed by UI labels. Download
offers, destinations, progress and finalization now enter that catalog before
history persistence. Private sessions retain completed rows only for their
shared session and never accept saved history or emit download-history writes.

Durable history uses atomic complete-record snapshots through the existing
storage coordinator. One shared write reservation coalesces progress across
windows; revision-aware acknowledgements cannot erase newer progress, and real
history failures wait for explicit retry. Transfer admission and controls no
longer depend on saved metadata readiness. Shutdown drains newer snapshots after
an admitted older write within the existing shared retirement budget.

Each primary, secondary and popup window now has a Downloads workspace. Typed
Qt variant rows carry filenames, state, progress and file/transport availability.
Known completed-file actions remain available during history refresh or failure;
Open still validates the real finalized regular file. Private command admission,
captured-profile transport dispatch and native file publication remain required
integration checks. Application tests pass 83 cases, core tests pass 154 and
storage tests pass 102; strict targeted Clippy passes for all three crates.
Six focused QML checks pass for invoking-UI
actions, unavailable history, completed files, stable delegates and private
session presentation. Fresh embedded production qualification is running in
`downloads-session-production-v3`; no native pass is inferred from these source
and application-level checks.

The first fresh production build and native test build pass. Seven actual
transfer cases pass, including progress, Cancel, Pause/Resume, foreign-profile
refusal, same-profile peers, allocator retirement and one-time acceptance. The
private case then stops on a fixture error: after verifying a visible private
row, it reads bytes from the primary model. That observer error is corrected;
the private completion and held-history cases were not reached and are not
claimed passed. The initial full source QML run passes 614 checks but fails its
forced-GC invariant because the launch omitted the required Chromium flag.

Further source review fixes secondary pause-signal observation, preserves the
session-only exclusion from durable IPC download events, prevents destination
replacement, and adds fair bounded persistence batches. The action capability
registry now checks a ready profile download session, instead of requiring
durable history for private-window controls. Native cases additionally require
private Pause/Resume and completed-file action admission without launching a
desktop application. `downloads-session-production-v3` is the current fresh
qualification; its full source QML phase now passes all 615 checks with the
documented forced-GC flag, and its release build is live. V2 was deliberately stopped
before rebuilding to correct a failed preflight lint; its termination is not a
product QML failure. No current full production pass is claimed yet.

### Download folders, progress and current qualification

`downloads-session-production-v3` reached terminal success: all eight phases
pass, including 615 source QML/WebEngine checks, a fresh release engine/browser,
14 native Downloads checks and 14 existing download integration checks. Actual
private completed-file and history-busy progress renders were inspected. This
qualifies that source checkpoint; it does not prove the subsequent edits below.

The custom download folder now uses one validated type at startup and runtime.
It accepts an absolute folder path or the existing canonical tagged form, retains
`xdg-downloads`, and rejects relative paths, control characters and malformed
values before publishing an edit. Generated configuration documentation includes
the runtime command and reset. The main application/config/core/storage tests
pass 405 cases, and strict targeted Clippy passes for those crates and xtask.
The isolated candidate's dependency checks and exact integration hashes are
retained under `download-directory-candidate-v1`; its initial test-only lint
failure remains part of the evidence.

Rendered review exposed an almost invisible native progress fill. The manager
now binds the Qt Basic and Fusion progress palette roles to the browser theme.
Full-window image sampling catches the old failure and passes on the repaired
dark, light and minimal themes. ProductPolish and DownloadWorkspace checks pass
50 cases across Basic and Fusion. Their rate tests also verify that rapid
refreshes retain a useful speed baseline and that samples belong to each
window's workspace and clear on profile replacement. Native byte signals now
update the session catalog while the manager is hidden. Completed history writes
invalidate the saved cache on acknowledgement, preserving newer committed data.

`downloads-directory-production-v1` now reaches terminal success in all eight
phases: 620 source QML/WebEngine checks, a fresh release engine/browser, 15 native
Downloads checks and 14 integration checks. The actual transfer enters the
runtime-selected Unicode folder, retains that folder after a rejected relative
edit, and verifies finalized file contents and live backend byte progress. Source
and artifact hashes remain pinned. Production private completed-file and
history-busy progress renders were inspected: session-only copy and file actions
are clear, progress remains visible and Pause/Cancel remain available during
metadata backpressure. Separate source renders cover dark, light and minimal
palettes; these component fixtures do not establish whole-window/font coverage.

The remaining finalization audit confirms that the GUI completion handler still
copies and syncs files synchronously, and creates the final destination name
before its contents are complete. Reusing the generic worker without lifecycle
changes would either block retirement on its join or detach uncontrolled
mutating work. Owned background finalization, complete exclusive publication and
cancellation/retirement evidence remain required. Runtime query/userscript
consistency, private public-event exclusion, retry delivery and delayed
destination cancellation also remain open alongside all original seven areas.

### Typed setting-query feedback

The actual public CLI probe against the qualified directory browser reproduces
`get no.such.setting` losing its `NotFound` cause and returning a generic
`E_ENGINE`. The exact candidate now keeps the typed cause through support-command
dispatch and uses one shared validation path for UI and IPC. An unknown setting
names the requested setting and points to `:settings`; an invalid site address
asks for an HTTP or HTTPS address without displaying the internal `config.get`
method name. Explicit stale-window routing retains its own category.

Native feedback cases and the public CLI probe require unchanged navigation,
retention of an earlier accepted setting, and rejection of later commands after
the unknown setting. The obsolete string-only query-error accessor is removed.
`config-get-production-v1` is the fresh frozen qualifier; format/whitespace and
the release build pass. Its Rust test harness is compiling before strict Clippy,
native feedback and public CLI qualification. No positive runtime or whole-goal
claim is made from candidate integration alone.

### Public Downloads and registered-action consistency

The typed setting-query release builds successfully. Its first Rust harness
passes 502 checks, fails one obsolete assertion requiring a removed root-window
download action, and ignores one check. The assertion is corrected to retain the
manager/backend boundary without requiring obsolete glue. The native feedback
group then passes 23 checks and the actual public CLI probe passes, including
specific missing-setting errors, retained earlier settings and stopped chains.
These runtime checks use the immutable release with a verified subsequent
`cfg(test)`-only edit; corrected Rust harness execution and strict Clippy remain
part of the next qualification.

`download-public-before-v5` proves private download event exclusion through the
actual authenticated endpoint, under both private-query configuration values.
Each case verifies complete owned payloads for a normal transfer, a private
transfer and a second normal transfer. One continuously drained subscriber must
receive both normal completion barriers and no other download IDs. The same
probe reproduces live and completed queries returning `E_BUSY` while an actual
SQLite writer is held, a registered download action returning `E_ENGINE`, and an
explicitly permitted private query returning no rows. V1's long socket path,
V2's legacy-code expectation and V4's pre-schema manifest seeding are retained
as fixture failures; they are not transfer or product-event failures. V3 proves
the event boundary before script setup is added.

The integrated repair uses the existing profile-session catalog for public
queries and registered download subjects. Saved-history completeness is
reported separately from session-row readiness; private results still require
configuration permission and explicit opt-in. Query sequence watermarks account
for shared-session updates from another owner. GUI, command and public action
paths preserve typed manifest denial and missing-subject errors. Registered
action discovery uses one captured capability projection, separating download
session readiness from durable storage. Existing stored-library subjects retain
their storage requirement. Public and native regressions require actual script
input, private allowed/denied policy, foreign-profile refusal and a held writer.
The strict public probe is registered in CI and its query contract is documented.

The added native script fixture compiles against the immutable earlier release;
this is compile evidence, not repaired API runtime proof. The first strict
`downloads-api-production-v1` check rejects the capability record's four boolean
fields and missing test imports before any release/native phases. The record
now uses the existing `PrivacyKind`, and the explicit test imports are fixed.
`downloads-api-production-v2` passed strict release adapter Clippy, a fresh
release build, 18 native Downloads checks, 23 action-feedback checks and the
public CLI probe. The native script cases prove actual session-row lookup while
history is held, private allowed/denied policy and foreign-profile refusal. Its
public live query also returns the real 256 KiB progress with incomplete history.
The public registered-action call then fails because native routing injects a
window context that the userscript handler rejects as an unknown field. The
same diagnostic appears in the retained before-V5 script result: that earlier
public failure proves the routing defect, not the later session-row lookup.

Registered actions now decode bounded context through the shared command
schema, reject local-source spoofing and apply the same live window/profile and
context-selector policy as command-form actions. The public regression adds
malformed/source/foreign-profile/stale-window refusals and explicit normal-owner
execution while a private window is focused. The fresh frozen
`downloads-api-production-v3` passes strict adapter Clippy, its release/native
builds, 18 native Downloads checks, 23 action-feedback checks, the public CLI
probe and the expanded public Downloads probe. Both private-query settings
complete all three owned 1 MiB transfers, exclude private events between normal
completion barriers and reject nine malformed/unauthorized routing contexts.
Actual script input and operation IDs prove held-history lookup and explicit
normal-owner execution while a private window is focused. Private manifest
denial and foreign-session lookup retain their typed errors. The corrected
Rust harness passes 503 checks with one ignored. All eleven V3 phases pass with
source and artifact hashes retained. This qualifies the Downloads/API and typed
get/action batch; all seven original areas remain open for the final combined
audit.


Two retained native V2 Downloads renders were inspected directly: held-history
progress keeps the actual 256 KiB/1 MiB transfer and controls visible, while
completed private Downloads explicitly says history is not saved and exposes
Open/Show in folder. This limited render review is recorded separately from the
still-required whole-window and compact/large-font walkthrough.

The owned startup recovery probe additionally reproduces generic failures for
unmarked selected custom/XDG data and an overlong local connection path. All
selected-data and unrelated-default sentinels survive; no reset is run. These
are remaining recovery-copy defects under the original status/error audit.
The Retry audit also confirms that current callers acknowledge JavaScript
completion rather than the exact native download offer. The isolated public-Qt
V2 experiment passes a before negative plus private, persistent, bodyless-page
and cross-origin-redirect download cases (seven QtTest checks including setup
and cleanup). The current approach navigates the actual user page and offers no
download; the helper preserves the page and profile cookie and saves the exact
owned payload. Redirect handling refuses fallback navigation, rebases controlled
HTML to the redirected origin, then starts the final download without duplicate
payload fetching. Qt reports this fallback as LinkClickedNavigation; a
RedirectNavigation-only check would miss it. V1 used an incorrect prototype
navigation-decision property and failed this case; V2 uses the public reject()
method and modern profile prototypes. This is approach-validation evidence,
separate from the still-required bounded request-token/native-offer integration,
retirement, timeout and redirect-loop checks. No browser Retry fix is claimed.


## Page-preserving download requests and Retry

The request pipeline now uses a bounded controlled view in the actual invoking
native profile. It adopts that view without activating the window and admits
only its exact controlled HTML during setup. HTTP(S) redirect fallbacks are
rejected, bounded and rebased without loading remote page content. The original
requested URL is retained for later Retry, including a fresh redirect. Source
pages and the browser's tab model are not used as download initiators.

A native offer must match the current request token, captured document/profile
and live owner before acknowledgement. The helper stays alive through a
pending destination decision and is released after an accepted transport takes
over. Authentication and certificate requests use the original page/window's
existing prompt ownership. Cancel request, navigation, operation cancellation,
profile replacement and retirement invalidate the request; late callbacks
cannot claim another window. Entering or leaving an input mode does not cancel
a background download preparation. Preparation operations have actual terminal
outcomes, and their completion describes an offer rather than a completed file.

Retry reservations belong to the validated profile session. Dropping an
unbound reservation releases it; an actual owned replacement holds it through
transfer completion. Peers cannot double-retry the same source. The typed retry
projection distinguishes unavailable, idle, preparing and running states;
`retry_pending` and `can_retry` expose the corresponding public availability.
Unusable saved addresses have an explanation instead of an enabled failing
Retry button. The manager can cancel preparation, and its help now distinguishes
preparation, transfer and desktop-file actions.

Evidence is deliberately split. Application Clippy and the first two shared
reservation regressions passed; the final unavailable-source test is added but
awaits the combined run. Source QML passed 25 checks before the final pending
and eligibility changes. An exact source-component run passed seven Qt checks
with actual native profiles, cookies and files; its ownership/backend boundary
is mocked and does not qualify BrowserUi. It caught the initial loadHtml guard
blocking Qt's controlled data URL, which is now accepted only when it decodes
to the exact packaged HTML. The first adapter strict check found three minor
lints; those are corrected, with later operation/eligibility edits still to
qualify. The native observer compiled before its final popup/redirect additions.

The `downloads-initiator-production-v1` qualifier passed full source QML
(621 checks), the current component (7 checks across 5 scenarios), strict
application/core Clippy and application/core tests (86 application, 5 integration
and 154 core tests). A source audit then found rejected rapid download hints
falling through successful activation teardown. The owned qualifier and all
its descendants were deliberately stopped before release compilation; its
adapter Clippy phase is stopped, not passed. These earlier source checks do
not establish production integration.

The rejection now preserves hint mode and its unconsumed selection. Registered
hint actions also retain the installed action's readable label instead of
overwriting it with an internal ID. Two native regressions drive actual hint
typing: a held-before-headers request rejects a second download, then permits
Enter to retry the retained selection after cancellation; a registered action
uses a producer barrier to verify its friendly notice and actual hint context.

The fresh `downloads-initiator-production-v2` qualifier will reuse V1's passed
QML/component/application/core evidence only after exact source/runner checks,
then run fresh adapter checks, release/native builds, the expanded native
Downloads/feedback groups, public CLI/Downloads regressions and the adapter
Rust harness. Native cases require
real cancelled-transfer Retry, redirect re-entry, private/secondary/popup/native
start owners, cookies, bodyless source pages, HTTP authentication, held payload
completion after helper disposal, before-header cancellation/navigation/retirement
and delayed destination cancellation. The public probe separately requires a
held request's operation cancellation and subsequent native-offer/file barriers
without changing source tabs or focused owner. These integrated runtime cases
remain unproven until that qualifier finishes. Original finishing responsiveness,
startup recovery copy and all seven areas remain in scope.

V2 passed strict adapter Clippy and built fresh release browser/engine artifacts
and native observers. Its Downloads run passed the direct, native-start, Retry,
redirect-Retry and private cases, then stopped in the secondary-window fixture.
An isolated repeat and an instrumented observer found the source page URL set
and loading false while its native main frame was not yet valid. The fixture
now waits for that actual frame before issuing scripts and reports script
admission failures immediately. No production startup defect is established
by that failed probe.

The two real hint regressions also ran against V2. They caught a second
rejection teardown in QML after Rust had correctly retained the session; QML
now keeps that rejected selection and displays the notice. The friendly-action
observer selected the wrong menu entry because arrow navigation skips disabled
rows. It now waits for the menu to open and observes its actual current index
while using arrow keys. These corrections are source changes awaiting fresh
qualification, not passing runtime claims. `download-initiator-native-diagnostic-v2`
rebuilds the native observers against the immutable V2 engine to diagnose the
remaining download rows and friendly action before recompiling packaged QML.

The corrected menu observer then reached the intended action and exposed an
actual policy mismatch: registered link actions applied saved-history eligibility
to the raw link before their existing URL redaction. A normal `#comments` link
was rejected before its action could start. The shared link-context boundary now
validates, sanitizes and then checks eligibility, retaining redaction of
fragments, credentials and secret query fields. A policy regression covers
those fields and rejects executable/data/control-containing URLs; the native
observer still selects an actual fragment link and expects its sanitized context.
This Rust change requires fresh strict and production qualification.

The next native diagnostic also found the direct-page observer captured its
expected address before command completion, and its popup row tried to construct
a popup without a native request. The observer now waits for the known requested
address and a valid native frame, and opens its popup through a real WebEngine
button click. The secondary redirect and bodyless rows passed in that diagnostic;
its remaining rows stopped at the invalid popup setup and are still unproven.
`download-initiator-native-diagnostic-v3` isolates the corrected download observer
against immutable V2 runtime bytes before the next full production build.

That corrected native diagnostic finished with **28 passed and 3 failed**.
Direct/native-start, actual cancelled-transfer Retry, redirect Retry, private,
secondary, bodyless, real popup, owner retirement and destination cancellation
passed. Cancellation and navigation before response headers removed the helper
but did not close the still-pending connection while the shared profile lived.
The HTTP authentication case never reached the owner prompt. Those failures are
retained as requirements; the download integration is not ready.

An isolated actual-Qt component experiment reproduces the HTTP 401 failure with
the correct profile cookie and no authentication callback, outside BrowserUi.
Keeping the helper alive until payload bytes did not fix it in a separate
candidate experiment; that candidate was not applied. The positive five
component cases still pass in both experiments. Exact Qt 6.11.2 download,
WebContents and login sources are being reviewed alongside the runtime proof;
the native API investigation is still open. The hint rejection test now also
releases its held producer explicitly and verifies that the late native offer
has no owner and creates no row, before Enter retries its retained selection.
Its connection-stop requirement remains in the dedicated failing download rows.

`downloads-initiator-production-v3` will run the new source QML/Rust checks,
fresh release/native build, hint/action feedback, public probes and Rust harness
before the complete Downloads group. The known failures stay in that final
group and must not be hidden by successful checks of the independent hint fixes.

## Late download-source crash and startup recovery qualification

`downloads-initiator-production-v3` passed source QML (621 checks), strict
adapter Clippy, release/native compilation and the real registered hint action,
then crashed while releasing a cancelled request's delayed binary response.
The captured Qt metadata identifies the failing getter as
`QQuickWebEngineDownloadRequest::view`; its QtWebEngine 6.11.2 implementation
dereferences a missing source adapter. A public production-browser reproduction
also exits with SIGSEGV (-11) when the cancelled response arrives after a second
download is admitted. Immediate release can race deferred page destruction and
miss the crash. Both the failing runs and their immutable artifacts are retained.

The registry now captures the source once through an isolated native guard.
It uses the matching installed private SDK declaration, with an unmodified,
licensed Qt interface and recorded provenance for the one header absent from
the installed SDK. It checks the running WebEngine version before accessing
private state, returns null for missing sources and preserves the existing
captured profile/window ownership policy. It never substitutes the active
window. Unsupported builds reject downloads with usable update guidance.

The standalone `download-source-native-v2` regression passes seven checks,
including real delayed binary responses after actual source destruction in
regular and private profiles, and exact-source checks with live pages. It uses
the production guard and disposable profile/XDG roots. This regression is now
part of `cargo xtask check`. The public download probe uses binary cancelled
responses and the explicit second-offer barrier. Fresh production qualification
passes source QML (621), strict browser/adapter/IPC Clippy, CLI/IPC tests
(110/34), release/native builds, action feedback (25), public CLI checks and
both private-query variants of the causal public Downloads probe. The failed
preliminary runner build exposed a QML registration restriction on final QObject
subclasses; that declaration was corrected before these passing runs.

The adapter Rust harness then passed 501 checks, failed three obsolete source
assertions and ignored one. The assertions demanded the unsafe direct view
getter, read the activation contract from the wrong component, or assumed a
particular Rust match-arm formatting. They have now been repaired; actual
native rapid-hint typing and pure typed command/policy tests retain the
behavioral coverage. `download-source-followup-v2` now passes fresh strict
adapter Clippy and the complete Rust harness (503 passed, one ignored). Early unsupported-runtime
admission now settles the operation before creating a helper or looking up a
source, and known native offers are handled idempotently before source lookup.
The added component regression passes in the full 622-check QML suite.
The updated compiled production resources still need runtime qualification. ADR-0002/spec/capability documentation now record the narrow
SDK exception. A disposable staging run of the actual package function
verifies the full license/NOTICE/provenance at their installed paths.

The reviewed startup recovery changes are also applied. Unknown or incompatible
selected roots retain the refusal/preservation behavior, but explain how to
choose a fresh empty root or compatible browser instead of suggesting a reset
of the wrong folder. Invalid local connection addresses have a typed
configuration error with shorter-path recovery guidance; raw filesystem detail
is excluded from the public CLI error. Fresh strict validation passes for the
browser, adapter and IPC crate, and their CLI/IPC suites pass 110 and 34 checks.
`startup-recovery-public-v1-result` also passes 14 actual CLI process scenarios
against the fresh frozen release browser: text/JSON unknown/incompatible roots,
custom/default paths, startup/client socket failures, stable exit codes,
preserved selected/unrelated sentinels and no raw path leakage. This actual
process regression is now checked into the application harness and CI; its
maintained version passes all 14 scenarios in the fresh follow-up run. Complete download qualification
remains open.

An independent actual-Qt fetch experiment passes seven checks: profile cookies,
native HTTP authentication, binary ArrayBuffer conversion and real connection
closure on abort before headers. A public interceptor additionally observes
the complete controlled document URL, including its opaque path token, as the
fetch request's first-party URL. This permits investigation of explicit
controller leases without guessing the owner by target URL. Its cross-origin
redirect check deliberately records the unresolved CORS restriction. This is
an ignored research fixture, not a production transport replacement,
file-streaming solution or resolution of the remaining native failures.

A separate unchanged-source follow-up completed the full native Downloads
group: 28 passed and three failed. The before-header cancellation/navigation
connections remain open while their profile lives, and HTTP authentication
never reaches the owner prompt. These tests remain required. No nullable-source
crash recurred. Frozen artifacts, source hashes and terminal phase results are
retained under `download-source-runtime-followup-v1`.

The next public-API research fixture (`fetch-download-redirect-prototype-v4`)
passes 14 checks across regular/private profiles. It registers the exact
approved request against the complete opaque controller URL, blocks a changed
target before its GET, then completes the admitted redirect from a new controlled
document. It also verifies profile cookies, HTTP authentication, binary bytes,
real before-header abort and revoked-controller refusal. An earlier fixture
incorrectly blocked the controlled document itself; the next run established
that these fetch redirects report NavigationTypeLink, so navigation-type
classification cannot provide the correlation. Both failed runs are retained.

This remains research, not production integration. The loopback ports share a
host/site; cross-site/SameSite/secure-cookie equivalence, existing network policy,
service workers, TLS/client certificates, streaming/backpressure and the actual
Rust operation/owner lifecycle still need implementation and qualification.
The three native production failures and responsive file finalization remain
open. All owned qualification/research processes are terminal at this checkpoint.

## Asynchronous saving and explicit-download transport — 2026-10-18

The finalization worker prepares the complete file off the GUI thread using a
held destination-directory descriptor and an exclusive hidden temporary file.
It synchronizes file contents there; the GUI validates the captured session,
owner and destination immediately before exclusive publication. An existing
file is preserved. Cancellation and owner retirement revoke publication without
joining a blocked worker. The directory entry itself is not separately fsynced,
so these checks do not establish crash durability of the final rename.

`download-finalization-production-v2` passes the actual held-fsync application
regression (seven QtTest checks, including setup/cleanup). Input and the event
loop continue while sync is held; completion verifies the 1 MiB payload;
Cancel/retirement remove temporary files and do not publish; collisions retain
the preceding file and a useful failure. Its broader run remains 28 passed,
three failed and five separately exercised finalization rows skipped.

The explicit-download replacement uses a browser-owned isolated document in
the captured Qt profile. Fixed scripts provide a cancellable fetch, bounded
binary chunks and native profile authentication. A bounded Rust worker writes
staging bytes; no network or Qt object crosses into that worker. Controller
leases admit each exact target through the existing interceptor policy. A
changed target is stopped before its request, reviewed for redirect limits and
HTTPS downgrade, and resumed from a fresh controlled document only if admitted.
Ordinary native page downloads retain the guarded native source path.

The service-worker research initially had a fixture navigation race; correcting
the actual document-readiness barrier is recorded in
`fetch-download-policy-prototype-v8-result`. It passes 20 checks. The site really
intercepts its ordinary page fetch while the synthetic controller receives the
real payload. The same run retains cookies, auth, cancellation, redirect and
binary-integrity checks. Loopback origins do not establish every cross-site
cookie or authentication policy; maintained production tests remain required.

The integrated scope also fixes finishing counts to the captured window and
releases obsolete rate samples. Same-profile peers retain control routing to
the original owner. Ordinary user notices now explain changed selections,
history boundaries, temporary-window failure and blocking/selector recovery.
Late renderer and Site Doctor completions are rejected without replacing newer
feedback; user-invoked recovery still explains its failure.

`download-stream-production-v1` freezes the integrated source and records
625 passing QML checks. Its remaining phases must pass before this work is
considered complete.

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
