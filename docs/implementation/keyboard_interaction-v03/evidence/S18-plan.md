# S18 implementation plan and qualification record

S18 extends the existing read-only `interaction.status` projection and Site
Ledger. The local view is refreshed only when `:site-status` opens or the user
explicitly refreshes it. It shows mode, focus category, shortcut policy and
source, bounded page-key count, native-prompt category and last outcome,
scroll-policy status,
and recent action-error category counts. It does not expose tab/window IDs,
frame paths, URLs, typed values, page labels, raw key mappings or audit IDs.
When policy is site-sourced, the local snapshot also reports the matching rule
ID for shortcut policy and page-key mappings. `binding-explain` shows the
requested key's actual owner, route reason, binding resolution, policy source,
and QML-special handling in status feedback without retaining the supplied
keychain in that feedback.

The status is assembled from current BrowserUi facts; it does not infer page
ownership. `bindings.explain` remains the per-keychain source for exact routing
owner, reservation, QML-special handling, binding resolution, and matching
policy/site-rule provenance. The status error summary accepts only C18's
bounded category vocabulary and maps every other category to `other`.

The source-versioned site-rule example in `examples/new-settings.toml` uses
settings implemented by the current config loader. It is not loaded by Ferric,
uses only the reserved `.test` fixture domain, and has no live qualification.
No interaction profile or adapter bundle is enabled or shipped as qualified.
It declares no adapter IDs; the 2026-10-12 review covers schema parsing and
validation only, while live qualification remains not-run.
Removing the example rule disables it; unsetting a value returns that setting
to inherited precedence.

## Test mapping

- T025: engine QML/native-insert gates exercise Escape recovery and both window
  status/help surfaces; no live physical Wayland keyboard result is inferred.
- T042: Rust unit coverage verifies bounded allowlisted error summaries remove
  action/operation IDs and tokenized URL text. Engine/QML gates cover the local
  Site Ledger surface and privacy boundaries.
- T055: `interaction.status` is included in IPC capability discovery, remains
  read-only, and is documented alongside its bounded response fields. Full
  workspace and documentation checks validate the typed contracts.
- T056: not run. No authorized disposable live-site accounts/documents were
  available for Docs/Sheets/Slides, mail/chat, editor, GitHub/Reddit, YouTube,
  or board/canvas qualification.

The native Site Ledger was not manually qualified with a physical compositor
input path. Offscreen QML and WebEngine results remain separate evidence.
