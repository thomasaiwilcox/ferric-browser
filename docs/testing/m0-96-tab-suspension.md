# M0-96: Engine-approved tab suspension

Date: 2026-09-16  
Status: in-progress  
Requirement: PROFILE-005

## Implemented slice

Automatic memory-pressure discard remains disabled. Users can explicitly
freeze a hidden tab with `tab-suspend TAB_ID` or `action tab suspend TAB_ID`.
The request is staged until QML verifies that:

- the tab is hidden and its WebEngine view is active;
- Qt recommends the `Frozen` lifecycle state;
- the tab is not recently audible and has no attached DevTools view;
- downloads, prompts, capture sessions, and browser-owned operations are idle;
  and
- a bounded page-world probe finds no dirty or unknown editable state.

Only then does Rust commit `ResourceLifecycle::Frozen` and QML set
`WebEngineView.lifecycleState` to `Frozen`. Selecting a frozen tab automatically
resumes it through the same typed lifecycle path, and `tab-resume TAB_ID` is
available for explicit resume. Failed or unknown eligibility keeps the tab
active with an actionable status message.

Discard/reload behavior and native runtime qualification remain later slices.

## Verification

Core reducer tests cover hidden-tab-only freeze and resume transitions. Qt tests
cover the typed command/action boundary and QML lifecycle wiring. The full
workspace verifier and native multi-tab smoke remain integration gates.
