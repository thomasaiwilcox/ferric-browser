# S09 implementation outline and evidence map

## Lifecycle policy

Keep an ephemeral ApplicationWorld document identity per tab. A same-document
URL change immediately updates the current URL and Mixed site routing while
ending the active pass-through mode. Persistent user-entered Pass Through can
be restored only through a short-lived lease that matches the tab, exact
document identity, destination URL, original input epoch, suspended input
epoch, active owner, and current Normal mode. Pass Next Key has no restore
lease. A new main-frame navigation, renderer loss, mode change, tab/session
change, or unresolved identity invalidates the proof and leaves the browser in
Normal mode.

History API and fragment behavior is observed in WebEngine's ApplicationWorld,
whose state is isolated from MainWorld. The token is a document-generation
identity, not a URL or same-origin heuristic. QML also captures the probe epoch,
tab index and stable tab ID, then checks the settled URL, loading state, and
navigation state before asking Rust to complete the lease. A failed or stale
probe takes the existing conservative URL-change path.

## Planned source changes

- Add `crates/ferric-browser-engine-qt/src/page_document_identity.rs` to own
  the bounded, per-tab identity registry, restore lease data, and narrow unit
  checks. This small module is specific to S09's same-document ownership proof.
- Extend the existing focus-observer ApplicationWorld script with a
  per-document token and trusted `popstate` sequence, and version the script.
- Extend `FerricNativeTabContent.qml` to observe identities, suspend persistent
  pass-through before settling a changed URL, reject stale callbacks, and
  restore only after the exact same document is observed.
- Clear identities and leases at navigation, renderer, tab, session, profile,
  and view teardown boundaries.
- Add component tests for exact identity, replacement, and delayed callbacks,
  plus a WebEngine fixture for SPA URL changes and document replacement.

## Per-case capability result

| Case | Result | Evidence |
| --- | --- | --- |
| `history.pushState` | Supported: document identity is unchanged and Qt reports no main-frame navigation request. | `test_T026_document_identity_survives_spa_urls_and_changes_on_replacement` |
| Fragment update | Supported: document identity is unchanged and Qt reports no main-frame navigation request. | Same WebEngine fixture test |
| Full document replacement | Detected: replacement gets a distinct identity. Navigation start and renderer failure invalidate the stored identity and restore lease. | WebEngine fixture, registry unit checks, and source-boundary review |
| Identity mismatch or stale callback | Safe reset: no restore is issued; `beginNavigation` invalidates the delayed QML callback. | `tst_native_tab_content.qml` component tests |
| Pass Through vs Pass Next Key | Only persistent Pass Through may be suspended; one-shot and Normal never qualify for restoration. | Rust unit tests and mode/epoch guards |
| Tab switch, reload, renderer death | Safe reset and identity/lease cleanup. | Source boundary review and existing engine/native lifecycle coverage |

The available tests do not drive the full BrowserUi pass-through restore path
through a native WebEngine tab while also causing reload, renderer death, and a
tab switch. S09 therefore remains `implemented_unqualified`; no claim is made
that the native-Qt T026 state transition was end-to-end qualified. T027's
existing gesture fixture preserves the owner of held gestures across policy and
epoch changes, but does not change focus to a different WebEngine recipient
and exercise the next physical gesture.

## Validation

Run the QML/WebEngine suite and native engine suite through
`cargo xtask test engine`, the full repository gate through
`cargo xtask check --locked --offline`, strict workspace Clippy, formatting,
and the pack validator. Keep the expected pre-fix QML failures in
`logs/S09/baseline-engine.log` and the passing post-fix logs beside this plan.
