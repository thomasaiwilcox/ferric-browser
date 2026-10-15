# Product polish pass — 2026-10-15

Status: active. Branch: `polish/refined-browser-experience`.

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

## Audit status and remaining implementation

These remain part of the original goal, rather than deferred release promises:

1. **Site Ledger / Site Doctor / diagnostics.** The ledger layout, readable
   summaries, disclosure and consent changes above need their fresh compiled
   qualification. Audit the remaining diagnostics/export surfaces for readable
   entry points and truthful unavailable states. The baseline 900×700 render
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
   unavailable-state audit, including read-only action affordances, normal
   metadata-unavailable history and private-transfer completion feedback.
4. **Status/error messages.** Audit operation IDs, stale-target terminology,
   adapter/engine wording and internal state transitions in ordinary status
   notices. Preserve useful diagnostic detail without flooding normal use.
5. **All prompts.** Inspect permissions, authentication, certificates, sharing,
   downloads and shutdown with long/untrusted content. An isolated baseline now
   reproduces a long page message pushing action choices outside the shared
   dialog, and private-history transfer actions overflowing a 390-pixel window
   with 16-point text (`target/product-polish/prompt-before.log`). The same
   baseline passes immediate Loader unload without deferred focus warnings;
   do not assume shared focus callbacks cause the remaining provider-teardown
   warnings. Confirm exact scope,
   safe defaults and keyboard/pointer behavior at compact sizes.
6. **Whole-window finish.** Verify theme/readability, larger fonts, focus,
   primary/secondary/popup consistency, and investigate observed lifecycle
   warnings where they indicate real broken behavior.
   Secondary/popup theme and popup load-handler fixes pass production tests,
   including real installed page scripts. Remaining observations include
   teardown callbacks whose owner context is gone. Guide/start-page unload
   callbacks and the secondary `viewUi` reference are fixed in the current
   batch; verify the secondary fix in the compiled run. Investigate live
   teardown behavior rather than masking all warnings. Native
   management commands currently require the primary window; broader access
   and prompt ownership still need review.
7. **Final evidence.** Run repository-required gates, compiled production
   integration and a rendered whole-application walkthrough after the final
   source is frozen. Keep the installed browser and normal profiles separate
   from disposable test fixtures.

## Evidence so far

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
