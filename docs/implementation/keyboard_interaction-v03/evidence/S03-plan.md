# S03 implementation plan

## Write boundary

- Add a closed Rust `HintIntent` → `HintTransport` mapping in core and make the existing selection path use it. Candidate JSON continues to carry facts only.
- Capture hint configuration revision in `HintSession`; invalidate sessions and pending palette authority when effective configuration changes.
- Add a non-serialisable, one-shot local palette permit in the existing hint policy module. Bind it to window, Hint session, tab/generation/document, config revision, and a monotonic request ID. Mint it only from the direct QML Hint action-palette entry; consume it once when a palette action is selected.
- Reject IPC attempts to claim the trusted local UI or physical keyboard command source. Preserve CLI metadata needed by existing IPC clients, while keeping those origins outside the permit mint path.
- Add unit coverage for closed intent routing, permit owner/source/replay checks, unsupported pointer grammar, and IPC source spoofing. Add the narrow ADR-0013 amendment that documents the future local-only handoff boundary.

## Scope amendment

Add `crates/ferric-browser-engine-qt/src/browser_ui_configuration.rs` to the
write boundary. Effective runtime configuration is applied there through a
separate `set_config_json` path; it must advance the Hint configuration
revision and invalidate a pending palette permit/session just like startup and
profile configuration updates.

Also add `crates/ferric-browser-engine-qt/src/browser_ui_profile_setup.rs` and
`crates/ferric-browser-engine-qt/src/browser_ui_context_commands.rs` for the
startup/profile/context configuration setters that feed the same revision
invalidation hook. Without those call sites, an already-open palette could
survive a real configuration change made through those paths.

Also add `crates/ferric-browser-engine-qt/src/action_mapping.rs` for a focused
negative test: this is the generic action-executor mapping boundary, and it
must not resolve local Hint palette or pointer paths.

Add `crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeRequests.qml`
and its existing `src/tests/presentation_boundaries.rs` coverage for palette
dismissal. Closing a local menu must revoke its outstanding permit, while
selecting an item must preserve the permit until Rust consumes it.

Extract the existing long Hint selection path into
`crates/ferric-browser-engine-qt/src/browser_ui_hint_selection.rs`. The S03
owner/transport/permit checks belong beside that path, and extraction keeps
both cohesive Rust modules within the repository's 1,000-line module budget.

No pointer or scoped action is added to the visible palette or command grammar in S03; S04 owns the first user-facing pointer action. The permit is an internal authority boundary, not a serialized token or page capability.

## Test mapping

- T009: retain S02 payload/privacy tests and rerun the pure-unit and WebEngine suites.
- T010: pure-unit source/owner/replay tests and IPC route tests pass; native pointer handoff remains `not_run` until S04 exposes the action.
- T020: validate Hint session/config owner and consume the existing action-palette permit once; retain S02's direct map teardown test. Destination scope lease transfer remains outside S03 and is recorded as a qualification gap.
- T055: reject pointer grammar before effects; run repository manifest, IPC, command-registry, and documentation gates without adding user-facing command entries.

The only additional documentation path is `docs/architecture/ADR-0013-spatial-grid-navigation.md`, explicitly required by S03. No pointer adapter or Grid reducer changes are planned.
