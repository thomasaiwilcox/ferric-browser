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

## Remaining audit and implementation

These remain part of the original goal, rather than deferred release promises:

1. **Site Ledger / Site Doctor / diagnostics.** Replace primary raw fact IDs and
   JSON with useful summaries, keep inspection deliberate, clarify experiment
   names and outcomes, preserve private-site withholding and report consent,
   and verify scrolling when the panel is crowded. A 900×700 baseline render
   (`site-ledger-before.png`) shows diagnostic cards pushing experiment controls
   outside the panel and remediation text overlapping the footer. Reset host
   disclosure and reviewed-fix consent when their site/proposal changes.
2. **Profile/session previews and management feedback.** Review serialized tab
   descriptors, deletion wording and actual deletion scope, save failures,
   confirmation state, loading states and success/error visibility while a
   manager covers the status bar.
3. **Start, guide, libraries and navigation tools.** Audit labels, metadata,
   empty states, long titles/addresses, command explanations and learning
   affordances, including visible-target search and hint feedback.
4. **Status/error messages.** Audit operation IDs, stale-target terminology,
   adapter/engine wording and internal state transitions in ordinary status
   notices. Preserve useful diagnostic detail without flooding normal use.
5. **All prompts.** Inspect permissions, authentication, certificates, sharing,
   downloads and shutdown with long/untrusted content. Confirm exact scope,
   safe defaults and keyboard/pointer behavior at compact sizes.
6. **Whole-window finish.** Verify theme/readability, larger fonts, focus,
   primary/secondary/popup consistency, and investigate observed lifecycle
   warnings where they indicate real broken behavior.
   Secondary/popup theme and popup load-handler fixes pass production tests,
   including real installed page scripts. Remaining observations include
   deferred Guide focus callbacks after destruction, a secondary `viewUi`
   reference warning, and teardown callbacks whose owner context is gone.
   Investigate their live behavior rather than masking all warnings. Native
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
