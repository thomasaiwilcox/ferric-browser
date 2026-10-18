# QML JSON contract allowlist

Ferric presentation code normally crosses the Rust/Qt boundary through typed
properties or models. JSON is retained only where QtWebEngine itself returns
an opaque JavaScript value, where an intentionally opaque diagnostic/export
payload is displayed, or while a versioned page-script request/result contract
is being migrated. This document is the review allowlist for those exceptions.

## Opaque page-script request/result contracts

- Site-data clearing: `BrowserScripts.clearSiteData`, its polling result, and
  `site_data_clear_finished` use a bounded result object from page JavaScript.
- Hint collection, core interaction snapshots, activation, userscript actions,
  and detached-window handoff use versioned, bounded page-script,
  Rust-bridge, or cross-window result objects. Hint interaction JSON is capped
  by the 5,000-candidate session limit and keeps matching policy in Rust.
- Explicit native-pointer Hint preflight returns only bounded candidate facts
  and view-local geometry; the one-shot adapter request carries a request ID,
  session ID, surface stamp, coordinates, and fixed action enum.
- Scoped Grid handoff and geometry checks pass only the exact weak-lease
  identity, bounded CSS rectangle, viewport dimensions, and surface stamp;
  Rust maps that rectangle into WebEngineView-local logical coordinates before
  the existing native pointer adapter receives a dispatch request.
- The primary and secondary window runtime pumps use bounded selection, caret,
  editor, download, and page-evaluation request tokens with opaque page-script
  result objects. Rust validates each result before it changes browser state.

## Opaque diagnostics, export, and command payloads

- The site ledger and diagnostics snapshot are opaque diagnostic/export payloads.
  Permission metadata retains its `ready` compatibility field and includes a
  bounded status tag from the application authority: `available`, `pending`,
  `unknown`, `unavailable`, or `not-stored`. Only an actual pending read or
  mutation is described as loading; missing or failed reads cannot establish
  that the site has no saved choices. Private details remain withheld.
  Site actions re-read the existing diagnostic snapshot before using consent,
  comparing its site/document identity with the reviewed snapshot. Diagnostic
  summary rows, report readiness and engine-update notices use typed Qt
  properties or return values; preparing a notice must not parse or replace
  the reviewed diagnostic report.
- Saved-session preview site labels, addresses, pinning and muting use typed Qt list
  properties. Session preview errors use the existing completion receipt;
  previews do not serialize a presentation model into JSON.
- Command-line journey arguments are one-shot process-start inputs, parsed
  before normal browsing begins.
- Native-page navigation uses the existing tab-transfer descriptor and bounded
  content-history export/import contract (at most 4,096 entries). These opaque
  snapshots preserve native pages between engine history entries; the start-page
  tiles, guide, and preferences still cross the bridge through typed properties.

## In-memory permission request keys

`permissionQueueKey` encodes a fixed tuple of native window/view identities,
origin, capability and privacy scope into an unambiguous JavaScript map key.
The key stays in the bounded permission queue/cooldown bookkeeping; it is never
parsed, persisted, displayed or sent across the Rust/Qt boundary. This local
scalar identity encoding does not introduce a policy or presentation payload.


## Switcher catalog readiness and local identity

The Switcher publishes catalog readiness through typed Qt properties:
`switcher_sources_pending`, `switcher_sources_failed`,
`switcher_sources_retryable`, `switcher_sources_notice`, and
`switcher_sources_detail`. These describe the sources needed by the current
query scope, independently of journey reads and optional index construction.
The external `switcher.query` IPC response adds an `availability` object with
`pending`, `failed`, `retryable`, `notice`, and `detail` fields. It is computed
for that response and is excluded from cached result projections. Diagnostic
detail is bounded to 4,096 characters and remains behind a disclosure in the UI.

Result selection, pressed actions, and combined-source deduplication use local
length-prefixed scalar keys. Those keys are never parsed, persisted, displayed,
or sent across the Rust/Qt boundary. Row updates compare their typed fields;
they do not serialize a presentation model for equality or transport.

## Prohibited presentation payloads

The following formerly JSON-encoded policy/presentation data must remain typed:

- network hosts, host-to-list maps, cosmetic-rule records, and bypass lists;
- live blocking explanations and decision records;
- live window registry rows;
- userscript inventory, installation results, userscript actions, and page-userscript metadata;
- configured external actions and download desktop-action URIs;
- download activation requests (token and URL);
- profile-session download rows (Qt variant records with typed state, filename,
  progress, destination, retry availability and control/file-finalization flags);
- caret activation requests (token and operation);
- editor completion records (token, text, error, and stderr);
- JavaScript-evaluation requests (tab ID, world, and script);
- switcher activation requests (scope and query);
- additional startup URLs;
- journey graph edges;
- binding-help rows;
- switcher result rows, including their actions and ranking fields;
- settings and their mutation results, context choices, theme tokens, link-preview fields, and site-doctor
  status.

`tests::qml_json_contract_allowlist_is_explicit` records the current number of
JSON calls in the window runtime and native-page components (20 parses and 40
stringifications). Adding a new call requires a deliberate update to
this document and a review that it is an opaque page-script, diagnostic,
export contract, or the narrowly documented local scalar identity encoding—not
a presentation or policy model.

## QML write boundary

QML submits application changes through a named bridge request or a registered
command. It must not assign runtime properties directly. `status_text` is the
sole exception: it is transient native-callback narration for the visible
status surface, not application or policy state. The
`qml_submits_runtime_intents_instead_of_writing_runtime_state` architecture
test enforces this boundary.

Scope acquisition receipts are serialized inside WebEngine before crossing its QVariant result bridge, then decoded in QML and passed unchanged to Rust's strict receipt validator. The two scoped Hint/Grid handoff paths preserve explicit nullable parent-lease fields; a QVariant object conversion must not turn a valid null into an omitted field. These are bounded opaque page-script results, not presentation models.
