# Existing input ownership before Interaction routes

This baseline contains no Interaction runtime routing. The production router
and existing application own the decisions below. Tests named here are existing
executable fixtures; listing a test does not assert that every platform passes.

| Context | Existing behavior | Executable coverage |
| --- | --- | --- |
| Normal | Exact binding leaves execute; prefixes and counts retain browser ownership while awaiting completion or timeout | Core `input::tests::exact_leaf_executes_and_count_limits_are_applied`; `mixed_policy` decision-order tests |
| Insert / InsertLocked entry | Normal `i` enters transient Insert; `I` enters persistent InsertLocked | Core `insert_commands_are_immediate_and_case_sensitive` |
| Transient Insert Enter | Native Enter press/release reaches page; ordinary terminal release completes the captured epoch, modified/composing Enter does not inherit that exit | Native `nativeEnter`, `modifiedEnter`, `composition`, `compositionAlreadyActiveOnEntry`, `completionAfterRouterDeactivation`, `completionAfterWindowFocusChange` |
| Persistent InsertLocked | Ordinary editing remains native; Escape exits the editing mode | Native `ordinaryText`, `escapeExitsBoth` |
| Hint collection and loading | Browser owns label/query input; early input waits for real collection and invalidates on owner loss | Application `hintEarlyInput` primary/secondary rows, `hintRefreshRetainsInput`, `hintConfirmationFreezesPendingRefresh` |
| Mixed idle source | Existing explicit page keys precede idle ordinary bindings; reserved commands, active count and active keychain remain browser-owned | Core `first_key_decision_preserves_policy_mode_reservation_and_sequence_order`, `mapped_first_digit_beats_count_start_but_not_an_active_count`, `browser_prefix_mismatch_and_timeout_never_replay_to_a_mapped_page_key`; application `mixedReservedKeysAndUnmappedDelivery`, `mixedPageKeysAndBrowserSequence` |
| Mixed held gesture | Original page or browser ownership survives policy/mode-epoch changes through repeat and release | Native `mixedPageOwnerSurvivesPolicyAndEpochChanges`, `mixedBrowserOwnerSurvivesPageMappingChange`; application `heldPageKeyRetainsOwnerAcrossPolicyChange` |
| One-shot passthrough | `;v` enters; entry releases/modifiers/repeat-release artifacts do not complete it; selected real source key-up completes it; overlapping forwarded keys retain ownership | Core `passthrough_entry_bindings_resolve_in_normal_mode`; native `passNext`, `passEntryOwnsItsRepeatsAndRelease`, `passModifierDoesNotComplete`, `passEntryChainOwnsHeldPrefix`, `passHeldAndOverlappingKeys` |
| Persistent passthrough and rescue | `;V` enters; ordinary Escape reaches page, Ctrl+Escape rescues browser ownership | Native `passPersistentAndRecovery`; core passthrough entry test |
| Native prompt admission | Observable prompt bypasses Mixed classification | Native `nativePromptInputBypassesMixedRouting`; application `promptAcceptCancelAndNestedRestoration`, `productPolishPagePrompts`, `productPolishWindowPrompts` (current source-bound run passed) |
| Clipboard | ShortcutOverride and press retain the native/browser gesture owner; held browser gesture cannot leak its release | Application `clipboardShortcutOverride`, `clipboardNativeEditing`, `clipboardHeldBrowserGesture`, `clipboardOtherWindows`; native ownership harness clipboard callback |
| IME | Active preedit belongs to its composition receiver; ordinary Enter cannot inherit a prior noncomposing exit; destroyed receiver and lost releases are handled | Native `composition`, `compositionAlreadyActiveOnEntry`, `destroyedCompositionReceiver`, `compositionGestureAfterLostRelease`, `nativeEditorProxyFocusKeepsInsertAndComposition` |

Native fixture source is `crates/ferric-browser-engine-qt/tests/native-insert/`.
It links the production C++ router but uses harness admission callbacks;
production QML/Rust routing is checked separately by application tests. Its
constructed Qt events do not establish physical X11/Wayland/layout support.

Current router source passed 96 native fixture and 24 ASan lifetime/reentrancy
QtTest cases with zero failures/skips (`native-final-test.log`, detailed
`native/Testing/Temporary/LastTest.log`). Existing application Hint tests have
prior retained passes in `application-focused-detail.log`. The current production archive rebuild and selected source-bound application
rerun passed: search38, Hint51, action-feedback40, clipboard81 and startup4
QtTest cases, zero failures/skips, `router-application-detail.log` and matching
XML. Recovery4 and prompt8 cases also passed individually in the retained
`router-new-application-detail.log`; its aggregate failed status is preserved
because that initial run included an invalid facet-test setup. Corrected
foreign negatives passed6cases separately. Recovery covers primary-window
mode exit and fullscreen ending and observes no protected Escape keydown in
the page. No secondary recovery equivalence is inferred. P00-04 baseline evidence passed independent review. Enhanced routing and
physical/platform qualification remain open.

Intentional shared URL-glob compatibility correction: `*` remains a wildcard
even when the target path contains a literal asterisk. The prior matcher could
reject `https://x.test/*ba` for pattern `https://x.test/*a`. Existing site rules
and context routes can now match these paths according to the documented glob
language. Interaction href predicates use the same corrected state machine;
there is no new glob language or query/hash matching. Section13.4 of the plan
records review, red evidence and the required final release-note follow-up.
