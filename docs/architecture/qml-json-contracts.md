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
- The primary and secondary window runtime pumps use bounded selection, caret,
  editor, download, and page-evaluation request tokens with opaque page-script
  result objects. Rust validates each result before it changes browser state.

## Opaque diagnostics, export, and command payloads

- The site ledger and diagnostics snapshot are opaque diagnostic/export payloads.
- Command-line journey arguments are one-shot process-start inputs, parsed
  before normal browsing begins.

## Prohibited presentation payloads

The following formerly JSON-encoded policy/presentation data must remain typed:

- network hosts, host-to-list maps, cosmetic-rule records, and bypass lists;
- live blocking explanations and decision records;
- live window registry rows;
- userscript inventory, installation results, userscript actions, and page-userscript metadata;
- configured external actions and download desktop-action URIs;
- download activation requests (token and URL);
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
JSON calls in `Main.qml`. Adding a new call requires a deliberate update to
this document and a review that it is an opaque page-script, diagnostic, or
export contract—not a presentation or policy model.

## QML write boundary

QML submits application changes through a named bridge request or a registered
command. It must not assign runtime properties directly. `status_text` is the
sole exception: it is transient native-callback narration for the visible
status surface, not application or policy state. The
`qml_submits_runtime_intents_instead_of_writing_runtime_state` architecture
test enforces this boundary.
