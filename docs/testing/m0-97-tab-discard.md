# M0-97: explicit tab discard and reload boundary

Requirement: PROFILE-005 and STATE-008

Automatic memory-pressure discard remains disabled. Users can explicitly
discard a hidden live tab with `tab-discard TAB_ID` or
`action tab discard TAB_ID`.

Before Rust changes the tab's resource lifecycle, QML requires the public
QtWebEngine view to be active, to recommend `Discarded`, and to have no audio,
DevTools, downloads, browser-owned requests, capture, or other tracked browser
operation. A bounded page-world probe refuses dirty or unknown editable state.
Unknown eligibility stays active.

Rust then commits `ResourceLifecycle::Discarded` and the engine adapter applies
`WebEngineView.LifecycleState.Discarded`. The tab strip exposes the state and
keeps resume explicit. Selecting or resuming a discarded tab returns its engine
lifecycle to `Active` and visibly requests a page reload; frozen tabs resume
without that reload boundary.

The reducer test covers hidden-tab enforcement and restoration. Typed command,
action, IPC, QML static checks, the full locked workspace gate, and the
application build are the automated evidence. Native multi-tab lifecycle and
reload behavior remain runtime qualification work.
