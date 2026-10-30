# Window mode and phone passkeys

Active implementation plan, authorized 29 October 2026. Hot tabs are deferred. The user requested continued implementation through completion, with one combined release. Work continues in `work/window-passkeys` at `/home/tom/Projects/ferric-everyday-work/worktrees/window-passkeys`.

## Current state

The public native credential foundation is implemented and independently reviewed. Production installs its credential-only isolated-world boundary when phone passkeys are enabled. MainWorld interception, the process ceremony actor, provider execution and result delivery are still disconnected. No real credential or physical-phone result is claimed.

Single-page windows use the existing Quick backend and preserve logical page identity, exact captured navigation, profile ownership, drafts and native popup handoff. Generic Wayland background windows remain unmapped until selected. Allocation failure keeps usable local views; expansion beyond twelve windows asks before allocation. First-page closure retires that page while preserving the application services needed by surviving windows.

The window command foundation now routes counted keyboard selection, next/previous, indices and stable page IDs through actual native owners, preserving profile/privacy/context/group scope. Interactive continuations run on the selected owner and recheck document and active-tab identity after synchronous Qt callbacks. Actual IPC rejects ignored context selectors and completes selection operations on the native handoff receipt. Persistent error notices report stale or failed commands. The reviewed fallback case with two live local views passes on the current static archive.

Profile-wide saving and append restoration preserve groups, contexts, pin/mute/zoom/scroll metadata and drafts. New schema 5 reads older snapshots. Restored keeper metadata is retired after materialization so later context changes persist. Independent workers share capture-time, path-scoped write ordering; delayed old snapshots cannot overwrite newer aggregates. Independently configured owners join the same live checkpoint identity for the actual profile session.

A native test build defect was found and fixed: a browser build could refresh dependency artifacts while leaving the top-level engine archive stale, and promoting the archive could preserve an older timestamp so CMake skipped relinking. Native application/public-host builds now check workspace implementation freshness and use an archive-content SHA link dependency. Both stale-input and same-timestamp content-change regressions pass. Build the engine archive explicitly with `cargo build -p ferric-browser-engine-qt --lib` before relinking native harnesses.

Remaining window work includes broader mutations/ordering, native-popup group inheritance, detach/give/undo, exact compositor grouping and complete session/close integration. The public CLI currently splits literal command chains into separate RPCs; full continuation semantics across those requests still need completion. No full window-mode milestone or combined release is claimed.

There is no implementation-pack approval gate, amendment transaction, file-count restriction, attempt counter or A61 migration requirement. The historical goal metadata remains paused; this authorized implementation work continues independently of that machinery.

## Work to complete

- [x] Start from product integration `7eb1c94`, recovering queue candidate `517eb9f` and fixture repairs.
- [x] Prove public native ownership, captured navigation, lifecycle feedback and stale-request rejection.
- [x] Reuse Quick UI, services, profiles and popups; independently review isolated credential receiver, native/Core/profile association, installed-policy fencing and helper baseline.
- [ ] Complete window commands across actual native owners, including group selection/counts/previous/index, keyboard/actions/IPC chains, pin/mute/order, detach/give and scoped closed-page undo.
- [ ] Complete exact Hyprland grouping and focus using opaque mapping-time tags, PID/app ID corroboration, instance/stable-ID/address rechecks and guarded public Lua operations. Unknown or failed grouping must leave usable windows; never merge unrelated groups or change desktop configuration.
- [ ] Complete aggregate sessions/checkpoints, one live checkpoint identity per actual profile session, ordered writes across independent workers, group/context/order/selection restoration, retained-page context changes, first/last-owner teardown and the existing expansion confirmation.
- [ ] Connect the reviewed native facts to one process-wide ceremony actor, effective direct-network admission, supervised fixed-sibling helper, bounded result delivery, owned QR/account/cancel UI and production opt-in MainWorld shim. Preserve native authenticator fallback and immediate cancellation on close/quit/navigation/policy/profile/renderer changes.
- [ ] Have the authorized independent reviewer inspect the integrated credential path and resolve findings before real credentials are connected.
- [ ] Complete workspace/native/helper/security/packaging checks, independent RP verification and physical qualification. Update the plan and release evidence to the actual final results.

## Architecture and safety decisions

Keep internal tabs as the default. Window presentation, phone passkeys and compositor grouping are opt-in. The public Widgets owner/scene remains an alternative prototype; completing a Widgets backend migration is not required. Reuse one captured engine-request consumer and the actual Quick profile/view/native-window lifetime registry.

The notification-only channel in reserved world 42 supplies no page-controlled origin, owner, target or result destination. Native main-frame challenges, exact native presentation and actual Core/profile leases establish authority. Native provider choice owns focus. Installed interceptor ownership and nonwrapping policy generation revoke stale work, including changes from sibling windows. Queued document continuations must retain their original Core Target across asynchronous window selection.

Phone transport initially permits only effective policy-approved direct networking. Proxy, offline, unsupported or restrictive routes disable phone admission before connection. The helper uses assigned pinned relays, checked public sockets, normal original-host TLS and Bluetooth proximity; no proxy bypass, redirect, replay, remembered-device or PIN storage. Preserve existing Qt authentication for unsupported requests and native provider choice.

Keep durable session data normal-profile-only, safe to restore and bounded. Restore metadata describes unpresented pages; an actual presented window's current context takes precedence. Aggregate writes require capture-time ordering that survives per-owner queues and cannot let delayed old snapshots overwrite newer ones. Preserve prior valid generations on failures.

## Evidence and remaining device work

Logs and reproducible SDK configuration are under `/home/tom/Projects/ferric-everyday-work/builds/window-passkeys/`.

- `cf0f901`: trusted native transfer preserves Core page identity; 172 Core tests passed.
- `quick-association-r8`: 12 native credential cases passed, including early/late mediator installation, native/Core association, context and actual installed-policy cancellation, sibling policy changes, interceptor replacement, renderer loss and distinct profiles.
- `page-association-r8`: 6 production association and 13 window cases passed. Independent scoped foundation review closed with no outstanding findings; session and private-bus cleanup passed.
- `page-close-r9`: 16 Qt cases passed, including command close/cancel, first-page retirement and allocation fallback. Corresponding engine build and application link passed.
- `page-selection-r23`: the full command scenario passes (3 Qt cases including setup/cleanup): counted native keyboard navigation, previous/index/stable-ID selection, correct-owner script continuation, queued navigation, synchronous navigation and active-tab reselection rejection, standalone-group exclusion and allocation fallback. The constructor registration defect is fixed: unadopted child views cannot overwrite opener registration.
- `window-ipc-r21`: four actual production IPC cases pass, including context-selector rejection without mutation, peer Core WindowId resolution and native activation operation receipts. This is offscreen IPC coverage; native focus is covered separately.
- `page-session-component-tests-r17`: all 148 application and 115 storage tests pass, including independent-worker ordering. Storage/application all-target Clippy and engine library Clippy (`r22`) pass with warnings denied.
- `page-commands-r23` and `quick-page-commands-r23`: current-archive production association 6/6, window 18/18 and Quick boundary 12/12 pass with zero failures/skips. Both owned sessions exit cleanly and private-bus cleanup has no remaining processes. The independent reviewer closed the scoped window/session/continuation findings with no outstanding issues. Earlier failed/stale runs remain historical evidence and do not qualify changed code.
- `storage-page-sessions-r11b`: all 11 session tests passed, including schema 5 group/context/selection round trip and schema 4 migration. Subsequent ordered-writer changes passed the full component suite above.
- Helper baseline: 17 library + 4 binary tests, Rust 1.88 all-target Clippy, optimized build and disposable installation layout passed. 578 upstream tests passed (one existing ignored); advisory/license/source/provenance, PIN/caBLE and Noise-vector checks passed. Baseline independent review closed.
- Independent HTTPS RP verifier: 12 tests passed, including counterless zero/zero semantics and nonzero replay rejection. This is fixture verification, not physical authentication.

The user offered an iPhone and authorized a separate security reviewer. Android and a native security key remain unarranged. Physical tests require actual participation and independently verified RP results; synthetic/virtual fixtures cannot substitute for them. Missing device qualification blocks the combined release, not independent engineering progress.

Use a separate disposable Qt profile for the exact `https://passkeys.ferric.test:8443` RP, supplying its exact-SAN fixture CA before profile creation and scoped loopback mapping. Do not alter global DNS/CA trust or bypass certificates. The phone does not need to browse or trust the RP fixture. Before scanning, explain that a real disposable phone passkey may sync, then arrange phone/provider deletion; RP cleanup cannot delete that credential.

Final validation: workspace formatting, Clippy, tests and `xtask check`; focused production application/Wayland and credential lifecycle/hostile-input regressions; helper workspace and packaging checks; final integrated security review; actual device qualification. Do not push, publish or replace the user's installed browser.
