# M2-14 Site Ledger evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirements: DIAG-004 and SITE-001.
- Files: `crates/browser-engine-qt/src/lib.rs` and
  `crates/browser-storage/src/permissions.rs`.
- Observable result: read-only `site.status` returns a bounded, privacy-safe
  ledger for the active tab and document, and the interactive `site-status`
  command opens the same snapshot in native browser chrome.

## Evidence

- The response captures runtime tab, window, document, generation, profile,
  context, loading, and renderer identity, plus a normalized HTTP(S) origin.
- Private profiles redact URL, origin, and profile identity while retaining
  only the explicit private marker and runtime-safe tab facts; per-site blocker
  explanation, count, and decision-trail fields are also redacted.
- Facts carry provenance, scope, apply time, active/unknown state, capability
  status, and bounded remediation action IDs. Blocker counts and host-only
  explanations are reused without exposing request URLs, headers, cookies, or
  authorization data.
- The blocker fact includes the last 100 sanitized blocked or
  allowed-by-exception decisions for the active first-party host. Entries are
  memory-only, contain resource host/type, matched rule, decision, and a fixed
  reason, and are cleared at the QML document-load boundary.
- Current-origin durable permission decisions are listed without storing or
  returning the origin a second time in the rule payload. Session and pending
  permission collections are explicit and empty until their owning lifecycle
  surfaces are connected.
- IPv6 permission origins retain authority brackets during normalization, so
  `https://[::1]` remains a valid canonical origin.
- The QML Site Ledger panel shows the captured origin/identity, fact state and
  provenance, bounded fact values, and safe remediation IDs. Refresh and
  Escape/Close preserve the read-only boundary.
- Active Site Doctor state is included in the snapshot and shown as a visible
  status-bar badge, so a temporary experiment cannot be mistaken for normal
  browsing policy.
- Compatibility facts use the compiled reviewed registry and expose only
  bounded, scoped active IDs; the current empty registry cannot invent a
  workaround for an unqualified site.
- `cargo test -p browser-storage -p browser-engine-qt --locked --offline`
  passes, including origin-redaction and IPv6 normalization assertions.
- A fresh `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser
  --temp-basedir` startup reached the timeout without QML/runtime errors,
  confirming the native panel resource loads on the Wayland path.
- The registered `site-doctor blocking-bypass` experiment adds a current-site
  bypass only in memory, reloads the captured active tab, and automatically
  removes that bypass on success/failure. `site-doctor-undo ID` uses the same
  typed cleanup path, and tab close/profile reconfiguration clears it.
- The registered `site-doctor userscripts-off` experiment is offered only when
  matching page userscripts exist. It clears installed scripts for one reload,
  suppresses both browser/page injection phases, and restores normal matching
  after success, failure, navigation invalidation, undo, or close.
- The registered `site-doctor fresh-view` experiment opens the current safe URL
  in a new tab using the same profile, tracks that temporary tab as the test
  target, and closes it after success or failure. The original tab remains the
  captured subject and no profile or security settings are changed.
- The registered `site-doctor compiled-defaults` experiment reloads once with
  supported site-scoped settings ignored, so the engine receives its compiled
  defaults. It is offered only when the current site has such rules and
  restores the normal site-rule layer after completion, expiry, or undo.
- The Site Ledger's `Copy sanitized report` action calls a Rust-generated
  report with a fresh correlation ID and app/engine/platform facts, capability
  statuses, bounded blocker list IDs, and active experiment state. The current
  site host is disclosed only when the explicit `Include current site host`
  checkbox is selected; query strings, paths, cookies, tokens, form text, DOM,
  account identifiers, profile names, and private-session data are excluded.
- A successful blocker-bypass experiment creates a pending durable-fix
  proposal containing the exact generated `blocking.bypass_sites` override,
  before/after values, experiment provenance, security effect, and reload test
  behavior. The UI requires an explicit review checkbox before applying it
  through the existing typed `set` command and reloads the blocker policy;
  stale proposals are rejected if blocker settings changed meanwhile.
- Site Doctor experiments now carry a Rust monotonic 30-second deadline. The
  visible Site Ledger countdown and the 50 ms browser heartbeat expire the
  experiment through the same cleanup path as `site-doctor-undo ID`; navigation,
  tab/view close, and profile reinitialization remain invalidation boundaries.

## Limitations

The shared Qt profile interceptor callback does not include a tab ID, so the
decision trail is first-party-site scoped and exact tab attribution is reported
as unavailable. Decisions do retain the bounded source list ID for the matched
host rule (and exception rule when present), without retaining list URLs or
request paths. Engine capture/device state, installed userscript inventory,
populated compatibility-quirk entries, and service-worker/site-data controls
remain future slices. Those facts are
reported as unknown or unavailable rather than inferred. Userscript and
fresh-view results remain report-only because this build has no safe persisted
per-site userscript or compatibility setting.
