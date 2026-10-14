# Keyboard interaction implementation review

**Repair follow-up:** The three findings below have been repaired in source commit `5410c038409a835d271a08b42e96ea6dd434d6f9`. Permanent regressions and final verification are recorded in the [repair report](../implementation/keyboard_interaction-v03/evidence/review-repairs-report.md). The original review observations follow.

**Original review result:** The reviewed implementation needed three corrections before installation for broader testing. The native pointer adapter can send a press after an observed input invalidation, configured selectors do not reach the JavaScript consumer in its expected shape, and configuration reload does not activate the new reload-scoped selector settings. These are reproduced integration defects. Passing the existing suites does not establish that the implementation is bug-free or fully qualified.

## Comparison and scope

The review compares freshly fetched `origin/main` at `e9a576fd375927733b200f91a6387dea4698840d` with local HEAD `d9d3a1c0f87bc082d8e9836bd8e9f6f799e44c0f`. The branch is 69 commits ahead and zero behind. The complete difference contains 599 files, including extensive evidence logs; the crates account for 152 files, 21,730 added lines and 1,542 removed lines, including tests. The final source commit is `bc9284eb8a55f948f45846fa3f18635f8ae9c507`; HEAD adds documentation and evidence.

This is a self-review of the whole branch, with additional reproductions at the integration boundaries. Inspection covered Hint discovery and final actions, selector composition and validation, local interaction permits and command provenance, Mixed key ownership, prompt and document lifecycles, scope transfers, native pointer dispatch, spatial coordinates, overlay allocation, resource packaging and test orchestration. It is not an independent review or an exhaustive proof of every execution path. Production files were not changed, and no installation or push was performed. Existing untracked `aqtinstall.log` and root `tools/` were preserved.

## Slop diagnosis

### P1 Native pointer dispatch ignores invalidation between move and press

**Location:** [browser_page_pointer_adapter.cpp](../../crates/ferric-browser-engine-qt/src/browser_page_pointer_adapter.cpp), `FerricPagePointerAdapter::dispatch`, lines 249–273. **Confidence:** reproduced.

The move is sent synchronously. During its delivery, a Qt callback can change the target, block input, disable the adapter, resize the surface or deactivate the window. The adapter defines `currentSurface()` with the required checks, but the first press does not call it. Its outer condition only checks that the captured pointers remain alive. A release can consequently follow a press that should never have begun.

A small harness linked the production adapter and installed a window event filter that called `setInputBlocked(true)` during the dispatched move. It observed `active=1 blocked=1 moves=1 presses=1 releases=1`. The expected count after this observed invalidation is zero presses. The reproduction uses Qt Quick offscreen and no replacement adapter logic.

This violates C06's requirement to reject observed chrome/prompt/surface invalidation. It is distinct from the admitted, unobservable renderer targeting interval: the adapter already knows input is blocked. Both Pointer Hint and Grid share this adapter. The missing first-press recheck also exists on `origin/main`; this is an inherited adapter defect exposed by the new programme, rather than a newly introduced regression.

### P1 Effective selector JSON does not match the script contract

**Locations:** [browser_ui_navigation.rs](../../crates/ferric-browser-engine-qt/src/browser_ui_navigation.rs), `effective_hint_selector_snapshot`, lines 27–43; [PageInteractionScripts.js](../../crates/ferric-browser-engine-qt/qml/scripts/PageInteractionScripts.js), `ftHintSelectors`, line 134, and related replacement checks at lines 138 and 198. **Confidence:** reproduced through the packaged application.

Rust serializes each family as `{ "policy": { "mode": "replace", "selectors": [] }, "provenance": [...] }`, or with `policy: null`. JavaScript reads `policies[family].mode` and `.selectors` directly. It therefore interprets every production entry as an extension with no custom strings and falls back to built-ins. Extension selectors are ignored; replacement and empty-disable policies are ineffective. The same mismatch affects proxy replacement eligibility and fresh/final policy checks.

The release BrowserUi harness loaded shipped QML with startup configuration replacing both `all` and `scrollables` with empty lists. The active production snapshot contained those accepted nested policies, but `hint all` still produced **12 targets instead of zero**. This is not a validation failure or a pending runtime update.

The existing QML selector tests construct flat policy fixtures in `selectorSnapshot`, unlike the serialized Rust type. They pass while missing the production boundary. The application startup selector test checks configuration acceptance, but does not check discovery against that accepted configuration.

### P2 Configuration reload leaves selector settings pending

**Locations:** [browser_ui_runtime_config.rs](../../crates/ferric-browser-engine-qt/src/browser_ui_runtime_config.rs), reload publication around lines 620–624; [browser_ui_selector_config.rs](../../crates/ferric-browser-engine-qt/src/browser_ui_selector_config.rs), accepted reload around lines 265–267; [settings.rs](../../crates/ferric-browser-config/src/settings.rs), application timing at lines 1020–1025. **Confidence:** reproduced through the packaged application.

All seven new selector settings are documented as `reload`. Both reload paths publish through the ordinary configuration update, whose immediate projection only applies `live` and `navigation` settings. Reload values remain pending, and the reload paths do not activate them at this boundary. The underlying lifecycle machinery predates this feature; the new selectors expose the integration gap.

A disposable file contained `ui.font_size_pt=17` and an empty replacement for `hints.selectors.all`. After `config-reload`, the live font value changed and the status read `Configuration reloaded; 1 lifecycle change(s) pending`. The active selector snapshot still contained `policy: null`. The reload-scoped selector must become active after successful reload, while startup/restart settings retain their respective boundaries.

This defect is independent of the JSON mismatch: it is visible in the active Rust configuration before the script consumes any policy.

## High-confidence fixes

1. Check `currentSurface()` immediately before the first pointer press. If invalidation occurred during the move, terminate without beginning a button gesture. Retain a narrowly bounded cleanup release when a press actually occurred; do not send a new press or retry after uncertainty.
2. Establish one explicit selector snapshot shape. Either project a flat script DTO in Rust or unwrap `entry.policy` consistently in JavaScript, preserving provenance and null inheritance. Update every replacement-policy check together.
3. Add a configuration transaction for the reload boundary that applies validated reload-scoped values while retaining startup/restart deferral. Use it in both immediate and asynchronous accepted reload paths. Keep preparation, validation, persistence and publication ordering intact.

## Behaviour-sensitive changes

The selector repair restores authored configuration behavior, including empty replacement and proxy exclusions. Reload activation must preserve unrelated deferred settings. A broad activation of all candidate values would incorrectly apply startup/restart changes. Pointer cancellation must distinguish an action that never pressed from an in-flight gesture that needs cleanup and an uncertain terminal result.

Custom generic-div click rejection was also characterized. Existing tests explicitly require a selector match alone to grant neither click, focus nor native pointer authority. That rejection is excluded from the defect findings; fixes must preserve those action guards.

## Suggested refactor plan

Keep the corrections focused on the three boundaries. Mechanical cleanup, naming changes, abstraction removal and a broader architecture redesign are unnecessary for this review. First retain the failing behavior tests, then repair pointer invalidation, align the selector DTO, and implement reload activation. Improve error reporting only where it distinguishes cancellation from dispatch uncertainty. Re-run the affected tests before the full gates.

## Example patch

The first press needs the existing guard:

```cpp
const bool pressed = currentSurface()
    && QCoreApplication::sendEvent(dispatchWindow.data(), &press);
```

The release and terminal outcome must also account for whether a press was attempted. This fragment is a review suggestion, not an applied fix.

## Tests to add

- Production-adapter move callbacks that block input, disable the adapter, replace the target, resize the view and deactivate the window must produce zero subsequent presses. Separately verify cleanup after an already delivered press.
- Real BrowserUi startup configuration must drive discovery for extension, replacement, empty replacement, absent policy, and disabled implicit scrollable discovery. Consume the actual Rust snapshot rather than constructing a substitute flat fixture.
- Accepted selector changes through file reload, asynchronous syntax validation, runtime configuration and profile inputs must affect the intended lifecycle boundary. Failed/stale validation must preserve the previous configuration. Reload must not activate startup/restart settings.
- Verify final eligibility and proxy replacement exclusions with the same serialized policy used during collection.

The current isolated reproductions and logs are under `target/review-pointer/` and `target/review-application/`. They are ignored build artifacts, not committed regression coverage. The application harness copies the existing test driver and links the production release static library; its new cases are `reviewSelectorReplacement` and `reviewSelectorReload`.

## Vision and residual risks

| Programme area | Assessment |
| --- | --- |
| Activation and focus foundation | Shared validation, composed traversal and opt-in paired targets are implemented with fixture evidence. Native dispatch still needs the P1 correction. |
| Mixed ownership and recovery | Uses the existing router and exact-key policy; held-key and document/prompt tests support the local behavior. Physical input and platform IME qualification remain outstanding. |
| Declarative selectors | Rust layering and engine syntax validation exist, but discovery and reload are broken at the integration boundaries above. This part of the core vision is incomplete. |
| Regions, scoped Hint/Grid and Target Search | Implemented using the shared registry, ranking and owner-bound handoffs. Recorded application and coordinate tests support bounded fixture behavior. |
| Performance and teardown | Recorded optimized overlay p95 is 176–187 ms at the tested zoom levels; recorded 1,000-cycle teardown returns counters to baseline. These remain measurements of the specified offscreen environment. |
| Gated expansion | Double-click remains disabled; drag awaits evidence. Editor/frame/accessibility/label research remains bounded no-go work rather than promoted production capability. |

The ledger contains 20 `implemented_unqualified`, two `blocked_environment`, four `research_no_go` and two `complete` slices. The programme has substantial local implementation, but its own definition of done is not met. Physical Wayland input, authenticated live-site journeys, platform IME/choosers and human/screen-reader evidence remain required where specified.

The measured DOM/native observation interval also remains: all 20 recorded target-replacement trials clicked the replacement once. The contract admits that race and requires explicit non-default Pointer click with no retry. Fixing the observed Qt invalidation defect does not eliminate that separate limitation.

The application suite has previously timed out or lost positive cases under compiler contention, then passed while idle. Preserve the 250 ms product deadline; isolate timing-sensitive tests from compilation and give the suite a realistic total timeout. Teardown emits QML context warnings in the review harness; this review did not establish a user-visible teardown failure from those warnings.

## Verification

Fresh `QTWEBENGINE_CHROMIUM_FLAGS=--js-flags=--expose-gc target/release/xtask check` passed with exit code zero: 886 Rust and 160 QML cases, plus the repository gates. Its log is `target/review-check.log`. Fresh `cargo fmt --all -- --check` and source diff whitespace checks passed. Pack integrity and evidence bookkeeping passed; strict completion failed with 22 expected ledger-status errors, consistent with the outstanding qualification gates.

Fresh `ctest --test-dir target/native-insert --output-on-failure` passed, and the real release-browser `run_ipc_probe.py` passed restricted-chain rejection, URL preservation and Mixed routing checks. Logs are `target/review-native.log` and `target/review-ipc.log`.

The new native invalidation reproduction failed its safety expectation, and both new packaged application reproductions failed their functional expectations. Their logs are `target/review-pointer/repro.log`, `target/review-application/startup-repro.log` and `target/review-application/reload-repro.log`. The earlier completion ledger also records strict Clippy, 59 native component cases, 48 application functional cases, DPR checks and lifetime checks; those recorded passes do not cover the defects found here. No live-site, physical-input or daily-profile installation claim is made.

Correct the three findings and add permanent regressions before broader installation testing. Then run the full repository, native, packaged application, DPR/lifetime and CLI/IPC gates on the repaired source, followed by disposable-profile Wayland/manual journeys. Only matching evidence can close the remaining qualification gates.
