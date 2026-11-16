# Page-command cancellation

The existing `FerricBrowserKeyRouter` source record freezes the physical
identity, input epoch, binding-policy context, original key/text/modifiers,
resolved destination, native document receipt and one-shot owner on admission.
Continuation never resolves the mapping again. Native scan identity is preferred;
the Qt key identity is the fallback when no scan is supplied.

Every cancelled transformed or suppressed source stays captured until its real
release. A repeat release is not terminal. Cancellation retires output ownership;
it never promotes a collision loser or replays its press. Cleanup below means
at most one destination release, only if the original window, native receiver
and document receipt are still valid. It cannot target a replacement document.

| Trigger | Output disposition | Focused regression |
| --- | --- | --- |
| Policy publication/revocation | Attempt safe cleanup, stop repeats; retain physical capture | `compiledPolicyPublicationCancelsHeldSource`, application `policy-publication{,-once}` |
| Input epoch or explicit mode/route change | Attempt safe cleanup; invalidate one-shot completion, except already admitted overlap during one-shot completion | `sitePolicyNativeReentrantSetter`, `sitePolicyNativeProductionGesture:one-shot-overlap-after-normal` |
| Native focus leaves the original receiver | Cancel without cleanup to the new focus | `sitePolicyNativeCancellation:focus`, `completionAfterWindowFocusChange` |
| Window deactivation | Cancel without delivery; cancel one-shot, retain persistent mode; cancel pending recovery | `passDeactivation`, `productionRawAdmissionAndProtectedRecovery`, application `raw-recovery` |
| Source logical case/modifier transition on repeat | Attempt safe cleanup of frozen destination; consume changed repeat | `sitePolicyNativeCancellation:modifiers` |
| IME preedit begins | Attempt safe cleanup before forwarding preedit; consume held-source repeats/release | `compiledSourceStopsOnPreedit`, `compositionCommitPreservesText` |
| Native prompt takeover / router disabled | Cancel without delivering into prompt | `sitePolicyNativeCancellation:prompt`, `sitePolicyNativeProductionPrompt` |
| Navigation / same-URL reload | Invalidate document receipt, no release to new document | `sitePolicyNativeNavigationCancellation`, application `receiver-same-url-reload` |
| View transfer / target-window replacement | Cancel without delivery to new window | `deferredReleaseCannotReachAnotherWindow`, `sitePolicyNativeProductionViewDestruction` |
| View or router destruction during dispatch | QPointer guards stop continuation; orphan physical capture remains until real release | `sitePolicyNativeRouterDestruction`, `sitePolicyNativeProductionViewDestruction` |
| Focused frame replacement or unsafe/unknown observation | Revoke receipt, no continuation into replacement/unknown receiver | application `receiver-frame-replacement`, `receiver-closed-shadow`, `receiver-custom-editor` |
| Tab/Enter changes DOM focus within the same document | Keep frozen document route while its observation remains valid; no promise of original-element delivery | application `receiver-tab`, `receiver-enter` |

Lost-release recovery only captures a cancelled source with no active output.
Ctrl+Shift+Escape arms the existing protected capture; a fresh nonrepeat press
and real release of that stored physical identity retire it without executing
page/browser input. Autorepeat cannot retire a still-held source. Recovery does
not enter raw mode; the user explicitly retries. See
`productionRawAdmissionAndProtectedRecovery`, `mappedEscapePreservesProtectedRecovery`
and application `raw-recovery`.

Native receiver observation is asynchronous. Public Qt delivery does not lock
DOM focus between observation and submission. A script may change the focused
element within the same document; the implementation does not claim otherwise.
