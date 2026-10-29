# Window mode and phone passkeys

Active implementation plan, authorized 29 October 2026. Supersedes everyday-v1 execution machinery; historical evidence and failures remain archived. Hot tabs are deferred. One combined release is required.

## Milestones

- [x] Clean branch from product integration 7eb1c94; recover queue candidate 517eb9f including fixture repairs.
- [x] Real captured navigation through the public page host, real lifecycle feedback and stale-request rejection.
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

Branch: `work/window-passkeys`, worktree: `/home/tom/Projects/ferric-everyday-work/worktrees/window-passkeys`.

29 October: captured navigation passes through the existing production queue consumer and shared EngineRequests dispatcher to the native owner. Actual page load/title events update Rust state. Native host/owner/document stamps and single-use request serials reject stale execution and replacement-page replay; retired native targets cannot fall through to a Quick view. This remains an experimental integration seam, not a completed browsing backend.

Validation on current source: pinned Qt 6.11.2 dev library builds; native host and test C++ syntax checks pass; Rust formatting and diff checks pass. Resource manifest matches all 118 current resources. The new navigation test passes offscreen and on an owned native Wayland compositor. Existing T014e2s transfer passes on that compositor (zero failures/skips). T014e1 owner suite passes offscreen. Build/test logs and reproducible SDK environment are in `/home/tom/Projects/ferric-everyday-work/builds/window-passkeys/`. Queue Rust tests are running separately.

Next: finish the production credential-boundary proof early, then complete shared backend services/interaction, window presentation/grouping/restoration, helper transport/UI and final combined qualification. No full-feature or physical qualification is claimed.
