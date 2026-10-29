# Window mode and phone passkeys

Active implementation plan, authorized 29 October 2026. Supersedes everyday-v1 execution machinery; historical evidence and failures remain archived. Hot tabs are deferred. One combined release is required.

## Milestones

- [x] Clean branch from product integration 7eb1c94; recover queue candidate 517eb9f including fixture repairs.
- [ ] Real captured navigation through the public page host, real lifecycle feedback and stale-request rejection.
- [ ] Complete shared native backend: history/form reload, search/scripts/hints, input/chrome, services, popup and profile lifetime.
- [ ] Early production credential-only isolated-world boundary and real native provider choice; independent security review before credentials.
- [ ] Complete single-page window presentation, commands, exact Hyprland grouping, restoration and generic Wayland fallback.
- [ ] Complete supervised phone helper, direct-policy admission, credential semantics and accessible owned chooser/QR/cancellation.
- [ ] Final integrated regression/security/packaging qualification, physical iPhone/Android and native-key verification.

## Implementation decisions

Reuse Rust policy/runtime, public QWebEnginePage owners, and shared QML workspaces. Maintain one request consumer and exact page/profile/host/document ownership. Do not mix live opener chains between backends. Keep internal presentation default and phone/grouping opt-in. Phone transport permits only effective policy-approved direct networking initially; unsupported proxy/offline/restrictive policies disable it before connection. Existing qualified SDK/download guards remain.

Preserve native authentication, actual popup handoff and profile isolation. Unsupported/failed grouping leaves usable page windows. Session expansion above twelve windows requires confirmation. No hot-tab allocation or measurement is required; existing configuration groundwork is retained.

## Validation and release

Run focused tests per change, then workspace fmt/Clippy/tests, xtask check and engine/application/Wayland suites; test the separate helper workspace explicitly. Prove hostile frame/origin/message and document lifecycle rejection, helper bounds/network policy, native input/popup/first-host teardown, exact grouping/focus, sessions and private isolation. Physical iPhone, Android and native security key results must come from actual devices and independently verified RP results. Missing qualification remains pending and blocks the combined release. No publish/install over user browser.

Use ordinary Git review and actual test logs. No amendment transactions, file-count limits or attempt counters gate implementation. A61 drafts are preserved without further development.

## Progress

Implementation started on branch work/window-passkeys in the window-passkeys worktree. No feature or physical qualification is yet claimed.
