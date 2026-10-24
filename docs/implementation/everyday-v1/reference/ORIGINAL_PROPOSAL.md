# Ferric — Fast New Tabs, Phone Passkeys and Optional Window-Managed Browsing

**Research and design proposal · 15 October 2026 · v1.0**  
**Status:** Proposed work, not an implementation or security certification.  
**Ferric source baseline:** `af30f6fb889ba694bf9fd2d71bbab3779427882e` on `main`.  
**Ohm reference:** `ijt/ohm`; main resolved to `95640f6148d0ec436060825218a3e61916a44bc5` during research.  
**Platform baseline:** Ferric's dependency manifest qualifies Qt 6.11.2; the proposed first platform remains Linux/Wayland, with optional Hyprland integration. [F08]

## Executive decision

Implement the three capabilities as independent extensions of the existing browser, not as a second browser architecture:

1. **Hot tab:** a single, bounded, never-visited spare WebEngine view, adopted only when it matches the requested browsing environment. Retain the ordinary allocation path and keep the optimization only when measurements justify it.
2. **Phone passkeys:** explicit QR-based registration and authentication using a separately supervised Rust transport helper. Prototype a credential-only, isolated-world bridge using public Qt APIs; preserve native security-key operation and require a security gate before enabling the phone provider.
3. **Window-managed browsing:** a configurable one-page-per-native-window presentation with full Ferric keyboard capabilities. Add Hyprland grouping through the existing compositor adapter, without making Hyprland the owner of browser data or permissions.

**Default internal tabs remain the compatibility baseline.** None of these features should require users to adopt the other two. A hot-tab miss must be equivalent to ordinary allocation; an unavailable grouping operation must leave a usable page window; an unverified authentication boundary must leave the phone feature disabled rather than weaken validation.

This proposal is based on code inspection and primary API/standards research. No Ferric build, browser benchmark, physical phone ceremony or native-compositor test was executed for this proposal. Numerical budgets below are proposed acceptance criteria, not measured results. Proposed symbols, files and configuration keys are explicitly new design elements.

---

## 1. Product scope and non-goals

The product objective is to make normal browsing faster and less obstructive, while preserving a familiar keyboard-first experience. These additions must not turn the current refinement phase into a general desktop platform project.

The first release includes fast ordinary tab allocation; phone-based `get` and `create` operations on supported top-level secure pages; a choice between internal tabs and individual page windows; reliable page switching and optional Hyprland grouping; and ordinary session/reopen behaviour under either presentation.

It does **not** include predictive website prefetching, a background daemon or login autostart, a passkey vault, cloud credential synchronization, remembered-phone pairing, conditional passkey autofill, general cross-origin-frame phone authentication, live conversion of existing windows between presentations, arbitrary compositor plugins, automatic edits to Hyprland configuration, or versioned/branched browsing state.

Use existing commands and registries wherever their meaning still applies. A new feature must not silently widen the scope of a destructive command or reinterpret a privacy boundary.

## 2. What the research changes

| Finding from inspected sources | Consequence for this work |
|---|---|
| Ohm currently keeps the application process warm with zero windows; its startup code explicitly replaces an earlier hidden-spare-window approach. [O01] | Treat Ferric's hot tab as its own measured optimization. Do not claim Ohm's startup behaviour proves a spare tab will help Ferric. |
| Ferric already creates a `preparedTransferFallback` view without an assigned tab. [F02] | There is useful allocation precedent, but it is not a reusable hot-tab manager. |
| Ferric's WebEngine view contains navigation, prompt, history, userscript and UI callbacks; an unset tab index does not disable all of them. [F03] | Introduce explicit spare/live lifecycle roles and gate callbacks. |
| Secondary Ferric windows set `hint_chrome_available: false`. They are not equivalent full-feature browsing hosts. [F04] | Window presentation requires shared interaction components and behavioural parity, not merely more calls to the existing secondary-window factory. |
| Ferric already has bounded, optional Hyprland IPC and version-dependent dispatch handling. [F06] | Extend that adapter rather than create a competing integration based on blocking shell commands. |
| Qt 6.11's public WebAuth UX request handles account/PIN/retry/cancel interaction but exposes no QR/hybrid transport control or external-result injection. [Q01] | A phone QR display cannot simply be added to the existing security-key dialog. |
| Qt Quick WebEngine exposes `webChannelWorld`, allowing a channel to be placed in a selected JavaScript world. [Q02] | A narrow isolated-world credential bridge is a plausible public-API route, but its security properties must be demonstrated. |
| Ferric currently has a test forbidding WebChannel exposure entirely. [F07] | A credential bridge requires an explicit architecture decision and replacement security tests; agents must not casually delete the guard. |
| `libwebauthn` supplies hybrid transport and RP/origin validation facilities. `credentialsd` describes its browser integration as experimental. [L01], [L02] | Reuse transport/cryptography through a pinned helper dependency. Do not require an assumed universally available Linux credentials portal. |

### Interpretation

The hot tab is an optimization experiment. Phone passkeys are a security-sensitive compatibility feature. Window-managed browsing is a presentation/lifecycle feature with a prerequisite refactor. Their implementation costs and release gates should therefore be different.

## 3. Common architecture: preserve ownership, share the necessary host code

Ferric's documented boundaries place policy and application state behind Rust application/runtime APIs and leave platform effects to Qt adapters. Preserve this direction. [F01]

### 3.1 Existing versus proposed ownership

Keep `TabId` as the identity of a browser page, independent of how it is displayed. A page's document identity, history, permissions, privacy scope and pending operations remain browser-owned.

A native page host is a presentation/lifetime object. In internal mode one native window presents several pages. In window mode each ordinary native window presents exactly one page. Do not pretend that a native window is another tab hidden inside the original primary window.

Introduce only the coordination needed for these features:

| Proposed responsibility | Location and constraint |
|---|---|
| Open-target and presentation policy | Existing Rust application/runtime boundary; typed intents and effects. |
| Page-view factory and spare lifecycle | Qt adapter, with Rust policy/state where it determines authority. |
| Per-window interaction host | Reusable Qt/QML controller composition, parameterized by the actual page/window owner. |
| Profile/session services | Existing application ownership, extended so multiple hosts share services without reopening the same profile for every page. |
| Credential admission and ceremony state | Rust policy plus a very narrow Qt document-bound bridge. |
| Compositor commands and observations | Existing Hyprland worker/adapter, extended with typed capabilities and observations. |

The present primary/secondary ownership arrangements must be mapped before extraction. Shared services in this table are a **required outcome**, not a claim that the current repository already implements all of them.

### 3.2 Four invariants

**One owner per live page.** No duplicate authoritative page state in a tab manager and a window manager. Each callback carries enough identity to resolve the current owner; an old index is never sufficient after moves or closure.

**Profile lifetime is not window lifetime.** A shared Qt profile, profile lock, storage worker and configuration owner must outlive every page using them. Closing the first-created window must not invalidate other windows. Private profiles are released when their real session ends, not retained by a spare.

**Interaction state follows the real window.** Hints, grid geometry, native input, text search, selection and prompts use the correct page host. They must not call back into whichever primary page happens to be active.

**Preparation has no page authority.** A spare may initialize infrastructure but cannot create history, credentials requests, permissions or user-visible application state.

### 3.3 Keep the refactor narrow

Extract the existing behaviour before enhancing it. Do not create a universal plugin framework or rewrite the reducer. Share an interaction host and a page factory; retain existing policy modules behind them. First prove the old internal-tab behaviour still works with the extracted components.

## 4. Feature A — bounded hot-tab preparation

### 4.1 User experience

An ordinary new-tab command should produce an immediately usable input surface. When a matching spare exists, adoption avoids part of the page/view setup. When it does not, allocation starts immediately through the current cold path. The user never waits for the spare to finish preparing.

A hot tab can reduce browser-side setup. It cannot guarantee faster DNS, network response or a particular renderer-process reuse decision. In particular, Ferric's existing native start-page UI must be measured separately from WebEngine readiness. [F03], [Q02]

### 4.2 Pool policy

Use **at most one speculative spare per Ferric engine process**, with an explicit coordinator so multiple windows cannot each create their own spare. This is not one spare per window or per stored profile. Prepare for the most recently active eligible environment; evict an incompatible spare instead of growing the pool.

An environment key must bind the exact live Qt profile instance and generation, the actual privacy/storage partition, applicable context isolation, the configuration generation and the presentation host kind. A matching profile name is insufficient. Never change a prepared view to a different profile to satisfy an allocation.

For the first release, prewarm only normal persistent browsing. Private and ephemeral opens always use the ordinary path. This restriction can be revisited after measurements; there is no requirement to keep a private renderer alive speculatively.

Spare creation is opportunistic after the current interaction settles. It is not a requirement during startup, navigation storms, renderer recovery or session restoration. During a burst of opens, use cold allocation rather than scheduling a chain of speculative replenishments.

### 4.3 Lifecycle

Proposed state machine:

```text
Disabled
   └─ eligible policy → Preparing → Ready → Claimed → ordinary live page
                            │         │
                            └─────────┴─ invalidation → Evicted
```

The view has an explicit `Spare`, `Claiming`, `Live` or `Closing` role. This must gate side effects in addition to the existing stable tab/document identities.

While spare, the view has no real `TabId`, tab-list row, accessible page entry, live-window entry, session entry, history entry, journey node or active prompt lease. It does not appear in MPRIS, downloads, the switcher or tab counts. It cannot request focus.

Prepare a never-visited blank document only. No external URL, user-chosen start-page URL, favicon fetching, login session probing, predictor or preconnect is authorized. Ordinary page userscripts and the credential bridge must not be installed into the spare. Audit both per-profile and per-view injection paths so broad script matching cannot accidentally run on the blank document.

Readiness needs an observable bounded probe of the prepared document; merely receiving `Component.onCompleted` does not establish renderer readiness. Keep object-created, document-ready and presented/interactive measurements distinct. If Qt does not prepare the expensive portion without a mapped window, record that limitation rather than briefly displaying a hidden window to manufacture a result.

A failed preparation is discarded. Use bounded retry/backoff; repeated renderer failures disable prewarming for that environment until recovery or explicit re-enablement.

### 4.4 Placement in the two presentation modes

**Internal mode:** keep the spare beneath an inert, hidden container in the destination `QQuickWindow`, outside the active `StackLayout` and tab-view list. Adoption reparents only within that same Qt Quick window. Destroy/rebuild on a different destination-window request rather than making cross-window scene transfer part of the hot path.

**Window mode:** qualify a prepared, **unmapped** host and its view as a separate variant. It shares profile/application services, not a hidden user page. Map it only after claim. If this does not produce meaningful savings without compositor artefacts, keep hot preparation supported for internal mode and report ordinary allocation for window mode. Window mode itself does not depend on this optimization.

Do not consume `preparedTransferFallback` from the tab-transfer workflow. That is a transaction reserve, not spare capacity available to speculative opens. Shared factory code is acceptable; competing ownership is not. [F02]

### 4.5 Atomic adoption

At a user open request, resolve the destination privacy/context/profile and presentation first. Claim a ready matching spare once. Bind the actual page owner and generation, apply the destination's settings and interceptor, attach live callbacks/observers, and only then permit real navigation.

Queued callbacks produced while spare must carry their old role/generation and be ignored after claim. A bare `tabIndex == -1` check is not enough. The adopted blank page must not produce an extra back-history entry or become the page's apparent previous visit.

Never recycle a previously visited or closed authenticated page as the spare. Blank navigation is not a reliable substitute for constructing a clean never-used page object.

First-release optimization applies to ordinary browser-created pages. Engine-originated popup/new-window requests use their existing cold path and the actual Qt new-window request handoff. Do not replace a popup request with a URL assignment. [Q03]

### 4.6 Eviction and shutdown

Evict on profile closure, relevant configuration or privacy-generation change, renderer termination, graphics/surface recovery, application shutdown and available low-memory notifications. Never keep the application alive solely for a spare. No user service, autostart or change to last-window exit behaviour is included in this work.

`on` may request preparation, but it does not override these safety conditions. `auto` uses conservative eligibility and any reliable available resource signals. Do not invent an unreliable battery/memory heuristic or introduce frequent process scans on the UI thread.

### 4.7 Measurement and acceptance

Measure the same build with prewarming disabled/enabled. Use a local deterministic page and separately record: input-to-focus-ready; input-to-engine-ready; and input-to-first-useful-paint/interaction on the target document. Test individual opens, bursts and cache misses. Separate running-browser opens from cold process startup.

Record p50/p95 distributions, cache hit/miss reasons, whole-process-tree proportional memory, idle CPU and the number of remaining views after repeated creation/destruction. Include a constrained laptop and a less constrained machine; record Qt/Chromium, compositor, GPU/backend and scaling. Offscreen timing is not a substitute for native Wayland timing.

**Proposed adoption budget, not a result:** require at least a 15% and 15 ms p95 improvement in a predefined relevant readiness metric on the target constrained machine; no more than a 5% p95 regression in input-to-focus-ready or cold misses; and at most 96 MiB incremental steady-state process-tree proportional memory for the single spare. Require no sustained idle activity attributable to replenishment and no monotonically growing retained-view count over 1,000 cycles. Change these budgets only with a written measurement-based rationale, not to turn a failed result green.

The optimization stays off by default until it passes. Rejecting a net-negative spare implementation is a successful experiment, not a reason to alter Chromium isolation or disable security protections.

## 5. Feature B — phone passkeys

### 5.1 Scope and transport choice

Deliver the explicit “use a phone” QR flow for both sign-in and registration. The phone authenticates the user and performs the authenticator operation. Ferric is not a credential vault and does not synchronize private keys. The conventional hybrid flow uses proximity verification and an encrypted transport; newer protocol variants exist, so do not hardcode an assumption that every transport uses the same relay path. [W01], [W02], [L01]

Prefer a small packaged Rust helper built on a pinned, reviewed `libwebauthn` dependency. Let the library perform the protocol and cryptographic work. Initially qualify the ordinary QR-initiated hybrid path with actual iOS and Android devices; do not require the newer direct-BLE mode or remembered-phone facilities to ship this feature.

Maintain a provider seam but do not implement multiple backends immediately. A future `credentialsd` backend is possible; its current browser integration is described as experimental and should not become a mandatory installed service for Ferric. [L02]

### 5.2 Why this is not a change to the existing PIN dialog

Qt's public WebAuth UX request does not expose the required transport selection or external-result completion interface. Existing native security-key UX should remain intact. A new browser-side integration must capture eligible credential requests before handing a phone operation to a helper. [Q01]

Ohm demonstrates a helper-plus-shim approach. Its broker uses a custom fetch scheme, a page token and the engine's request initiator; it also accepts a top-origin field from the page request. Those are source facts, not a claim of a proven exploit. Ferric should not copy that admission boundary without review. [O02], [O03]

### 5.3 Recommended first prototype: isolated credential bridge

Use a **dedicated isolated JavaScript world** and a **credential-only per-view WebChannel**. Qt Quick exposes a world selector for its internal WebChannel transport. This makes the route plausible without private Qt APIs or a QtWebEngine fork, but it does not by itself prove origin, frame or lifetime safety. [Q02]

Proposed path:

```text
Website's publicKey credential request
    → narrow MainWorld compatibility shim
    → browser-owned mediator in a reserved isolated world
    → per-view credential-only Qt bridge
    → Rust admission policy + native provider choice
    → supervised phone helper
    → exact request/document completion
```

The MainWorld shim changes only public-key credential operations needed for this feature and truthful capability reporting. Non-public-key credential calls retain their native path. Preserve references to native WebAuthn functions so the user can choose the existing security-key provider. Use the original request data for that path rather than reconstructing it from the phone serialization.

The isolated mediator communicates with the shim through a narrowly defined message protocol. It treats all page-provided options and correlation identifiers as untrusted data. A page is permitted to request credentials for itself; it is not permitted to select the browser owner, profile, origin or destination of a result.

Reserve the world ID centrally after checking existing userscript worlds. The bridge object has only credential admission/cancellation/result methods. Do not publish `BrowserUi`, storage access, arbitrary JavaScript execution, process spawning, keyboard injection, general IPC or configuration methods. The reserved world must not be available as an ordinary userscript/jseval target.

The current blanket no-WebChannel guard requires an explicit, reviewed architecture exception. Replace it only with stronger targeted tests proving that MainWorld and ordinary script worlds cannot access the credential transport or general native objects. Retain the prohibition everywhere else. “A test was inconvenient” is not a valid reason to remove it. [F07]

### 5.4 Admission contract and first-release frame policy

First release: **eligible top-level HTTPS documents only** for the phone provider. An authentication popup qualifies if it is a separately owned top-level browsing page. Subframe requests stay on the native provider where supported; they do not acquire the phone capability through the parent. Separate test-only loopback allowances may be used in controlled fixtures.

Do not accept `origin`, `topOrigin`, `crossOrigin`, secure-context status or permissions-policy decisions from the page message. Obtain the facts in browser-owned isolated code executing in the actual main frame, bind them to native view/document state and validate them again before completion. Reject opaque origins and unsupported contexts.

Require a per-document channel handshake. A proposed implementation uses an isolated-world document nonce, a native generation-bound challenge and a fixed main-frame validation script. Bind the accepted result to the concrete view, page/document generation, profile scope and request ID. Old registrations and completions must be unable to bind themselves to a replacement document, including a navigation to the same URL or a back/forward-cache transition.

Validate message source against the window owning that mediator; reject other-frame sources and synthetic message events. Do not equate `MessageEvent.isTrusted` with a physical user gesture: a page can legitimately post messages. Human authorization comes from the appropriate browser-owned provider decision and authenticator ceremony, not from that flag.

The host URL alone is not sufficient proof of the caller's security origin. Likewise, excluding subframe script injection alone is not proof that a forged frame message cannot reach the parent. Test both claims adversarially. Evaluate the relevant permissions policy using supported engine-provided facts; an unavailable proof must not be interpreted as permission granted. [W01]

**Prototype gate:** before integrating real credentials, demonstrate on Ferric's qualified Qt build that these origin/document/frame and isolation properties hold. If they cannot be established with this public-API path, stop this backend with the feature disabled and document the missing primitive. Do not downgrade to a page-trusted token, console-message bridge or generic CSP-bypassing scheme to complete the task.

### 5.5 Provider selection and compatibility

When the feature is enabled and an eligible explicit request arrives, offer a keyboard-accessible browser-owned choice between **phone** and **security key/native authenticator**, limited to providers compatible with the request. Show the validated site identity. Do not start both providers simultaneously.

Choosing native calls the preserved native operation and existing request UI. Choosing phone starts the helper. Cancelling either must settle the original promise appropriately. An ambiguous failure after an operation may have happened is not authorization to replay it through another provider. The asynchronous shim/chooser can change activation timing: prove native fallback still works without manufacturing user activation, synthesizing trusted clicks or relaxing permission requirements.

Preserve request semantics: challenge bytes, relying-party ID, allow/exclude credentials, algorithms, user verification, resident-key requirements and applicable authenticator-attachment restrictions. A request that requires a local platform authenticator must not be silently fulfilled by a cross-platform phone operation.

No conditional mediation/autofill, remembered phones or related-origin expansion in this phase. Do not claim a platform authenticator exists merely because a phone is available. Do not reproduce broad capability stubs or unconditional cancellation of native Qt requests. A platform-specific compatibility workaround requires a reproduced fixture and truthful reporting.

Handle extensions according to an explicit conformance table. Unsupported optional extensions must not be reported as honored; required semantics must never be dropped. Legacy `appid`/`appidExclude` behaviour should use a correct supported path or documented failure, not a blanket copied stripping rule. Test BufferSource conversion and ordinary credential response properties/methods with real site libraries, including libraries which modify response methods.

### 5.6 Helper and native ceremony lifetime

Create one helper process for one ceremony. This isolates crashes and simplifies cancellation; it is **not** by itself an operating-system sandbox. Launch only the packaged executable at a validated path, never a webpage-provided path or an implicit arbitrary PATH search. Avoid secret material in arguments or environment variables.

Use a versioned, typed, bounded protocol over private process pipes. Native policy attaches the verified origin/context; page input cannot override it. Only a closed set of progress events, a QR payload/matrix and a terminal result/error may be returned. Bound input, output, decoded binary fields, nesting and progress-event frequency. Validate matrix dimensions before rendering; never render helper-produced HTML. Render the QR with a proper quiet zone and fixed high contrast rather than arbitrary theme colours. Keep site identity, progress and Cancel keyboard/screen-reader accessible; do not expose raw QR secrets in accessible descriptions or copy actions.

A practical initial envelope ceiling is 1 MiB per request and terminal response, with smaller field-specific limits derived from supported operations. This is a resource policy, not a statement of universal WebAuthn limits; oversized operations return a defined error. Fuzz both boundaries and test compatibility near the limits.

Allow one active credential ceremony per Ferric engine process initially. Reject a concurrent request clearly; do not queue invisible credentials prompts. Use an absolute bounded deadline—proposed maximum 180 seconds—with the site's timeout treated according to the supported WebAuthn semantics. The original AbortSignal, navigation, document replacement, renderer loss, profile closure and application shutdown cancel the operation. Merely losing compositor focus does not silently retarget it; hide or suspend its UI and keep ownership exact.

Teardown is asynchronous: cancel/terminate, then bounded kill if necessary. Do not block the GUI thread waiting for process exit. A late result cannot complete a new request or a new page. Registration with an uncertain terminal outcome must be reported as uncertain/failed without automatic re-creation; an authenticator-side credential might already have been created.

### 5.7 Security, privacy and network policy

The helper validates the relying-party relationship using the maintained Public Suffix List and the browser-verified calling origin. Missing validation data is a capability error, never a reason to accept the RP ID. Bundle or package the required list deliberately. Pin and inspect dependencies and retain all required notices. [L01]

Do not log challenges, credential IDs, user handles, raw options, signatures, QR secrets, returned assertions, PINs or credential payloads—even in a general debug mode. Native diagnostics may include a coarse failure category, provider version and capability status. Existing redaction and profile-privacy rules still apply. Diagnostic export must not accidentally include helper stderr.

Hybrid helper networking must honor the effective proxy and egress policy. It cannot bypass a restrictive browser policy because it uses a different HTTP/WebSocket stack. Unsupported proxy configurations disable the phone transport with an explanation; do not silently use a direct connection. Bluetooth permissions and relay failures must produce bounded, actionable errors.

Private/ephemeral authentication uses the same constrained provider without persisting pairing or credential metadata. Registration can create a persistent credential **on the phone** even from a private browser window; explain that distinction. The browser must not suggest that closing the private window removes an authenticator-side credential.

### 5.8 Validation required to ship

Prove end-to-end registration and subsequent authentication on an independently verifying test relying party. Verify the returned challenge, origin, RP-ID hash, flags/user verification and signature—not just that a QR code appeared or the phone displayed success. Exercise discoverable and allow-listed credentials, excluded credentials, cancellation, timeout and invalid RP requests.

Test at least one physical iPhone and one Android phone, recording versions and credential provider. Use representative live sites only with authorized test accounts; redact evidence. A flow proven on one provider is not a claim about every password-manager app.

Adversarial cases include forged origins and top origins, public-suffix RP IDs, lookalike/IDN hosts, sandboxed and cross-origin frames, disabled permissions policy, same-URL replacement, document-start races, restored documents, stale bridge registrations, helper crashes, output floods, cancellation races, native fallback and attempts to call the bridge from MainWorld.

A separate reviewer should examine the trust-boundary implementation before default enablement. A source review or offscreen test alone does not qualify phone passkeys.

## 6. Feature C — internal tabs or native page windows

### 6.1 Configuration and identity

Offer two presentation choices: **internal** and **windows**. “Windows” is the implementation name for one page per native window; it is not a claim that every Wayland compositor supplies tab groups. Hyprland grouping is a separate optional capability.

The initial setting is startup-scoped. A configuration reload records a pending restart rather than reparenting active pages. Keep internal mode as the default. Permit profile-scoped startup selection through the existing configuration system, but do not vary presentation per website or spontaneously change it when a compositor is detected.

A native window still belongs to Ferric. It retains hints, spatial navigation, scrolling, text entry, caret mode, search, downloads, security prompts and the normal command UI. The absence of a tab strip must not imply a reduced-function browser.

### 6.2 Required host parity

Extract the primary-only interaction components so each page host receives its own window/page identity and native input router. Eliminate primary-window assumptions in hint collection, frame providers, grid geometry, command/search bars, focus return, permissions and credential prompts.

The existing secondary window's `hint_chrome_available: false` is evidence that this is real prerequisite work. Simply setting it to true would not establish parity. [F04]

Prove all relevant operations in a second window, then close the original/root window and repeat them in the survivor. Shared service ownership must survive this test without leaking a hidden browser window as its lifetime anchor.

Use one existing browser engine process/session as appropriate, not one independently initialized Ferric process/profile per page. Separate private or deliberately isolated profiles remain separate according to existing policy; window grouping never merges their browsing data.

Keep **keyboard focus, selection within a host and actual visibility** separate. Several tiled page windows can be visible while only one owns keyboard input. Conversely, a page selected within its one-page host can be hidden behind another compositor-group member. Do not freeze every unfocused window or infer group visibility solely from `active_tab_index`. Feed only qualified visibility facts into existing resource policy; when visibility is unknown, choose the non-destructive behaviour rather than invent a new resource scheduler.

### 6.3 Command behaviour

| User operation | Internal presentation | Window presentation |
|---|---|---|
| New tab / open in tab | Existing internal page allocation. | New page window; request the originating eligible Hyprland group when available. |
| Explicit new window | Existing new-window behaviour. | New standalone page window; do not automatically treat it as a tab-group join. |
| Next/previous/numeric tab | Existing order. | Current observed group's Ferric pages in the same browsing scope; when ungrouped, same-scope page order. The UI/help must state the active scope. |
| Switcher selection by page ID | Select page. | Activate the native window containing that exact page. |
| Close tab | Existing tab/last-close policy. | Close that page host through browser close/unsaved-work handling. Never close the whole compositor group. |
| Native compositor close request | Existing window-close handling. | Same close policy and undo capture as the browser command. A forced process kill is outside graceful-close guarantees. |
| Reopen closed page | Existing browser reopen contract. | Reopen a page window using the existing recoverable state; group with a still-valid source group only when safe. |
| Pin tab | Existing browser pin semantics. | Retain browser pin/protection metadata. Do not translate it into Hyprland's sticky-window pin. |
| Move/reorder | Existing browser operation. | Adapt only where the meaning is exact and supported; otherwise return an explicit unsupported result. |
| Popup / authentication child | Preserve engine semantics. | Dedicated child/popup host; not automatically an ordinary grouped page. |

The internal `last-close` concept must not cause every close command in windows mode to leave a replacement blank window. The chosen window-mode close semantics are intentional and must be documented; unchanged internal mode retains its existing policy. Bulk close commands capture an explicit Ferric page set and confirmation. Unsupported bulk/group operations must fail rather than act on non-Ferric windows.

A background open must not steal focus. This is a qualification criterion for programmatic grouping, not a promise that every compositor API can satisfy it. On a backend that cannot honor a requested background operation, report unsupported before an unsafe dispatch; do not quietly convert it into a foreground open. Already-created pages must never be destroyed simply because subsequent grouping failed.

### 6.4 Hyprland adapter, not compositor policy in QML

Extend the existing optional adapter and background worker. Add capability discovery, exact native-host binding, group observations, activation and narrowly scoped grouping operations. Maintain a bounded event-driven cache, with a full snapshot on connection/reconnection. No high-frequency `hyprctl` subprocess polling or synchronous UI-thread socket waits.

Hyprland's current documentation exposes Lua-based dispatchers as well as its IPC/event transport. Group membership and movement cannot be assumed to follow an older copied command string. Validate the actual installed version and operation semantics. [H01], [H02]

Proposed adapter operations such as `activate_page_host` and `place_new_page_with_source` are **semantic interfaces**, not names of existing Hyprland commands. In particular, the documented group-entry operation is direction-based; do not invent an address-to-address group-join API. A prototype must demonstrate how the supported backend can target the intended group reliably. [H01]

### 6.5 Exact identity is a release gate

Do not equate `QWindow::winId()` with a Hyprland client address. Do not identify a target by an arbitrary webpage title, “the active Ferric window,” or shared PID alone.

Bind a concrete Ferric host and generation to a compositor-instance epoch and native address. Validate app identity and PID as additional checks, not sole identity. Addresses are ephemeral and must never become durable session IDs.

Investigate a public protocol-supported per-window identifier first. Do not assume Qt 6.11 exposes every compositor-specific tag facility. A bounded fallback prototype can use a browser-generated opaque bootstrap nonce as the window's initial title, match it before any webpage title is exposed, and immediately restore the normal title after correlation. This is not title guessing; it still requires explicit review, installed-version validation, collision/reuse tests and a no-visible-flicker result. If no exact, acceptable mapping can be qualified, ship generic page windows without automated grouping.

This gate is important because all page windows may share both process and app ID. A focus change between creation and IPC completion must not move a different page.

### 6.6 Safe grouped creation

Capture the originating page/window identity when the user issues the command. Create the new page independently of grouping, obtain its exact native identity, then revalidate the originating group, privacy/context scope and locks before placement.

Never use a global “the next Ferric window belongs here” flag. Concurrent opens, OAuth children, developer tools and delayed mappings can consume the wrong slot. Each placement request needs a browser-owned token and an exact source and destination.

Respect user-configured group locks and membership. Do not unlock groups, change global autogroup settings, alter workspaces, steal focus or move unrelated applications to simulate success. Do not import other applications as Ferric tabs. If users manually mix windows in a group, browser destructive commands still operate only on explicitly selected Ferric page identities.

Some placement methods require multiple compositor operations. Prefer a backend-supported serialized operation with revalidation. Where only focus/direction-dependent operations are available, require proof that they cannot affect an unintended neighbour; otherwise decline and leave the page standalone. A timeout following a potentially delivered toggle is an uncertain outcome: query state before retrying, since replaying a toggle can undo the desired result.

Compositor observation is authoritative for live grouping, ordering and focus. Ferric may retain restore hints, but it must not run a reconciliation loop that continually fights a user's regrouping decisions.

### 6.7 Sessions, restoration and degraded behaviour

Persist page/session data in Ferric's existing format or a deliberate compatible schema extension. Optional logical group hints may reference stable Ferric page identities. Never persist live native addresses as restoration authority.

When switching presentation at restart, preserve the original saved session and convert its presentation deliberately. Opening a many-tab internal session as many native windows should require a preview/confirmation for a large expansion; materialize in bounded batches and retain lazy loading where possible. A suggested confirmation threshold is more than 12 page windows, explicitly a product choice rather than a measured limit.

Do not promise complete DOM/form restoration where the existing session mechanism does not supply it. Do not replay POST bodies or repeat authentication ceremonies during presentation conversion. Unknown/unsupported group hints are ignored safely while page data is retained.

When Hyprland integration is unavailable but the Qt display connection remains healthy, retain page windows and disable grouping commands. On an actual compositor/display-server crash, use the browser's crash-recovery path; this proposal does not promise that Qt windows survive a broken display connection.

## 7. Proposed configuration and rollout

These are **new proposed keys**, not configuration accepted by the inspected browser today:

```toml
[tabs]
presentation = "internal"  # internal | windows; startup/restart scoped

[performance]
hot_tab = "off"            # off | auto | on; one spare is a fixed v1 limit

[passkeys]
phone = "off"              # off | on; opt-in after security/device qualification
```

Add a separate `off | auto` page-grouping setting to the **existing Hyprland integration namespace**, using its actual schema conventions during implementation. Do not create a second root-level compositor configuration object merely to match an illustrative snippet.

All keys use existing configuration validation, precedence, atomic commits, generated references and settings UI. Feature enablement is not permission to bypass a failed runtime capability check. Do not expose arbitrary pool sizes or helper paths to page content or add unnecessary public tuning knobs.

| Stage | Internal tabs | Hot tab | Phone passkeys | Window presentation / grouping |
|---|---|---|---|---|
| Development baseline | Default, unchanged | Off | Off | Off |
| Qualified opt-in | Default, unchanged | Available when tested | Available after security and device gates | Available after host parity; grouping separately qualified |
| Later default review | Remains supported | Consider `auto` only with measured benefit | Separate product/security decision | Never automatically replaces internal mode |

Document effective capability status and a short disabled reason. This is particularly useful when Bluetooth, proxy support or compositor features are missing. Do not bury a failed dependency behind a permanently spinning webpage or a silent no-op.

## 8. Implementation slices and dependencies

The following are reviewable work packages, not instructions to launch one enormous change. Existing repository instructions and unmerged work take precedence. Record the actual baseline before touching shared files.

| Slice | Deliverable | Dependencies and completion evidence |
|---|---|---|
| S00 — Baseline | Current code map, explicit ownership map, existing failing tests, native opening traces, dependency/Qt/Hyprland versions. | First. No claimed baseline pass without execution. |
| S01 — Page factory/lifecycle | Shared allocation seam; explicit spare/live role; identity-aware callbacks; no public feature changes. | S00. Current internal browsing and transfer behaviour pass unchanged. |
| H01 — Hot prototype | Single spare, atomic claim, cold fallback, eviction and instrumentation. | S01. Real hot/cold comparison, no fake visible tabs or network activity. |
| H02 — Hot qualification | Native timing/memory results, bursts, profile changes, crash/graphics recovery and lifecycle stress. | H01. Meets Section 4 budget or remains off. |
| P00 — Credential boundary proof | Threat model, ADR, isolated-world handshake prototype, native fallback, hostile-origin/frame/lifetime fixtures. | S00; uses the existing live-view path. No real credential handling until admission proof passes. |
| P01 — Phone provider | Supervised helper, typed protocol, native chooser/QR UI, transport capability checks, proxy policy and cancellation. | P00; attach through S01 lifecycle seam before product integration. |
| P02 — Authentication qualification | Independent relying-party verification, iOS/Android tests, security review and packaging/PSL checks. | P01. No broadened platform or frame claims. |
| W01 — Host parity/services | Reusable interaction host, shared service lifetime and safe first-window closure. | S01. Hints/grid/search/prompts tested in every host. |
| W02 — Window presentation | Startup setting, one-page hosts, exact command scopes, graceful close and generic-compositor behaviour. | W01. No Hyprland requirement for basic usability. |
| W03 — Hyprland grouping | Exact identity prototype, capabilities/events, validated placement and focus-preserving switching. | W02; feasibility investigation may begin after S00. Never guess targets. |
| W04 — Restore/integration | Session presentation conversion, group-hint fallback, popup semantics and cross-feature matrix. | W02; include W03 and qualified H/P paths where enabled. |
| S02 — Release gate | Documentation, diagnostics, dependency/licence inventory, native evidence and rollback exercise. | Each shipping branch's qualification slice; other branches may remain disabled. |

Run the highest-uncertainty proofs—P00 and compositor identity/grouping feasibility—early. They may invalidate an implementation assumption before extensive UI work is invested. Hot-tab measurement and these proofs can proceed in parallel after the baseline, but edits to shared page creation, profile services and input roots should be serialized.

No single feature blocks shipment of unrelated stability fixes. A failed gate must not lead an agent to disable tests, relax origin checks, change a privacy model or substitute undocumented private Qt APIs.

## 9. Source areas and proposed additions

Existing anchors, verified by the source review, include:

| Existing area | Intended use |
|---|---|
| `qml/components/FerricBrowserRuntimeChrome.qml` | Extract page-view construction and callback lifecycle; preserve native start-page behaviour. |
| `qml/components/FerricBrowserRuntimeServices.qml` | Reuse allocation/transfer knowledge; keep transaction fallback separate from hot spare. |
| `qml/components/FerricBrowserWindow.qml` | Replace secondary-host feature restrictions with shared interaction composition. |
| `qml/components/FerricBrowserRuntimeRequests.qml`, `FerricWebAuthPrompt.qml` | Preserve native WebAuthn handling and integrate correctly owned provider UI. |
| `src/hyprland.rs`, `src/background_workers.rs` | Extend optional asynchronous compositor integration. |
| Existing core/runtime/application and configuration registries | Typed policy, exact ownership and generated user references. |
| `src/tests/presentation_boundaries.rs` and Qt/application fixtures | Replace overly broad structural guards only with explicit new contracts and executable tests. |

Paths above are relative to `crates/ferric-browser-engine-qt` unless otherwise stated.

Suggested new module names—not existing APIs—are `hot_page_policy.rs`, a small `page_host` controller, `credential_policy.rs`, `credential_bridge.{h,cpp}`, credential script resources and a `ferric-browser-passkey-helper` package. Prefer existing compatible modules when they already own the responsibility. Do not create a crate merely for each noun in this document.

## 10. Cross-feature acceptance matrix

| Test family | Required result |
|---|---|
| Internal/window presentation × hot disabled/enabled | Identical navigation, keyboard and privacy semantics; only allocation timing changes. |
| First window closed while another survives | Remaining hints, grid, commands, downloads, credentials and persistence still work. |
| Profile/context change while spare prepares | No cross-profile adoption; old completions have no authority. |
| Phone request from a recently adopted hot page | Only the actual navigated document can obtain a ceremony; spare state cannot leak through. |
| Two page windows with concurrent credential requests | Correct owner-bound UI; no credential/result delivered to the other window. |
| Popup authentication and `window.open` | Actual Qt request handoff retained; opener/child communication tested. |
| Background open with focus changes during mapping | No focus theft or grouping of a different page; unsupported path is explicit. |
| Group locks, mixed applications, detached source | No unrelated move, close or unlock; requested page remains recoverable. |
| Bluetooth/helper/proxy/PSL missing | Clear bounded capability failure; ordinary/native authentication remains usable. |
| Renderer loss, navigation and helper completion races | Exactly one terminal outcome; stale replies ignored; no automatic credential replay. |
| Application shutdown with spare/auth/grouping in flight | No orphan helper, profile kept alive only by spare, duplicate close or lost accepted storage work. |
| Corrupt/older session or unsupported compositor | Preserve data; reject invalid fields; ignore unsupported placement hints safely. |

For each enabled feature, record unit/fixture, composed application, native Wayland and physical-device evidence separately. A source-string assertion can enforce an architectural rule but cannot qualify behaviour. A Qt-generated offscreen key event does not prove compositor-delivered physical input. A QR scan does not prove a correct WebAuthn result.

## 11. Principal risks and decisions not to take

**Security integration:** no browser-owned bridge in MainWorld, no page-supplied origin authority, no disabling certificate validation, and no blanket cancellation of native WebAuthn. A dedicated-world channel is still a new attack surface and must justify its exception.

**Window integration:** no one-process-per-tab shortcut, global “next window” flag, broad focus/PID guessing, duplicated primary UI or automated compositor reconfiguration. These would undermine the goal of predictable everyday use.

**Performance:** no disabling sandbox/site isolation, speculative external pages or private-profile retention to create favourable timing. One bounded spare is enough to test the hypothesis.

**Scope:** no live presentation migration, credentials manager, universal desktop portal framework or expanded 2.0 session model. Keep failures explicit and the existing browser usable throughout.

### Alternatives considered

| Alternative | Decision |
|---|---|
| Ohm-style always-running daemon | Separate product decision; not needed for already-running Ferric's new-tab optimization. |
| Multiple spare tabs per profile/window | Reject initially: multiplies idle cost before benefit is demonstrated. |
| Add only a QR panel to native Qt WebAuth UX | Insufficient public API for the required transport/result handoff. [Q01] |
| Copy Ohm's custom scheme broker unchanged | Reject: Ferric needs its own reviewed document/frame admission and bounded helper boundary. [O02] |
| Chromium extension as the sole credential provider | Not the default route: Qt's extension manager excludes off-the-record profiles, and Chrome API availability does not establish equivalent Qt extension support. [Q05] |
| Require `credentialsd` | Defer: useful future provider, but an unnecessary deployment dependency for the bounded initial feature. [L02] |
| Patch/fork QtWebEngine immediately | Defer: first test the public-API path; a fork imposes a separate maintenance/security-update burden. |
| Direct compositor support before host parity | Reject: more windows with missing hints is not a finished Ferric mode. |

## 12. Definition of done

A user can continue using internal Ferric tabs unchanged. On an eligible configuration, a new page adopts a spare without visible bookkeeping or added history, and measurements show the optimization earns its idle cost. On a qualified phone-provider setup, a user can register and authenticate from a supported site with an independently verified result and without weakening native authentication. In window presentation, each ordinary page has the same keyboard capabilities as internal Ferric; Hyprland can manage qualified groups without becoming the owner of the browser's sessions, privacy or permissions.

The work is not complete merely because all three settings exist. Completion requires the corresponding user behaviour, failure behaviour, measurable evidence and documented limits.

---

## Sources and evidence register

All sources were inspected on 15 October 2026. Ferric links below are frozen to the reviewed snapshot. Third-party documentation describes API availability, not a completed Ferric implementation.

### Ferric source

- **F01 — Architecture boundary:** [docs/ARCHITECTURE.md][F01].
- **F02 — View allocation/transfer fallback:** [FerricBrowserRuntimeServices.qml][F02], including `prepareTransferFallback`, `removeTabViewAt` and `tabViews`.
- **F03 — Page view and callbacks:** [FerricBrowserRuntimeChrome.qml][F03], `webViewComponent` and embedded native content controller.
- **F04 — Secondary host and hint restriction:** [FerricBrowserWindow.qml][F04], including `hint_chrome_available: false`.
- **F06 — Existing compositor integration:** [hyprland.rs][F06].
- **F07 — Existing native/page authority guard:** [presentation_boundaries.rs][F07], `page_authority_boundary_has_no_web_channel_or_privileged_page_object`.
- **F08 — Qualified dependencies:** [packaging/dependencies.toml][F08].
- **F09 — Existing configuration enums:** [config_model.rs][F09], including last-close policy.

### Ohm implementation references

- **O01 — Warm application/IPC handoff:** [main.cpp][O01].
- **O02 — Scheme broker, origin handling and helper supervision:** [PasskeyBroker.cpp][O02]. Inspected blob `92b2b43dddd82075ddf196396e38795243c7ecf6`.
- **O03 — Hybrid helper and RP preparation:** [passkey-helper/src/main.rs][O03]. Inspected blob `54f9674760347b7b1d1d525be21c8d74a95a6306`.

### Qt 6.11 documentation

- **Q01 — Public WebAuth UX surface:** [QWebEngineWebAuthUxRequest][Q01].
- **Q02 — QML view, profile/lifecycle and channel-world properties:** [WebEngineView][Q02].
- **Q03 — Engine-originated new-window handoff:** [WebEngineNewWindowRequest][Q03].
- **Q04 — Request origin versus page/frame identity:** [QWebEngineUrlRequestJob][Q04].
- **Q05 — Extension support and private-profile restriction:** [QWebEngineExtensionManager][Q05].
- **Q06 — WebChannel asynchronous transport API:** [Qt WebChannel JavaScript API][Q06].

### Authentication libraries and standards

- **L01 — Hybrid transport, PSL and dependency requirements:** [libwebauthn README][L01]. Inspected README blob `7fbb386395d6c6c75109a286083d952a79eef136`; implementation must pin its selected library revision separately.
- **L02 — Linux credential service status:** [credentialsd README][L02].
- **W01 — Origin/RP, client data, request/response and policy requirements:** [WebAuthn Level 3][W01].
- **W02 — Hybrid transport:** [FIDO CTAP 2.3 specification][W02].

### Hyprland interfaces

- **H01 — Current dispatcher and grouped-window operations:** [Dispatchers][H01].
- **H02 — Request/event sockets and window events:** [IPC][H02].

[F01]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/docs/ARCHITECTURE.md
[F02]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeServices.qml
[F03]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-engine-qt/qml/components/FerricBrowserRuntimeChrome.qml
[F04]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-engine-qt/qml/components/FerricBrowserWindow.qml
[F06]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-engine-qt/src/hyprland.rs
[F07]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-engine-qt/src/tests/presentation_boundaries.rs
[F08]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/packaging/dependencies.toml
[F09]: https://github.com/thomasaiwilcox/ferric-browser/blob/af30f6fb889ba694bf9fd2d71bbab3779427882e/crates/ferric-browser-config/src/config_model.rs
[O01]: https://github.com/ijt/ohm/blob/95640f6148d0ec436060825218a3e61916a44bc5/app/src/main.cpp
[O02]: https://github.com/ijt/ohm/blob/95640f6148d0ec436060825218a3e61916a44bc5/app/src/PasskeyBroker.cpp
[O03]: https://github.com/ijt/ohm/blob/95640f6148d0ec436060825218a3e61916a44bc5/passkey-helper/src/main.rs
[Q01]: https://doc.qt.io/qt-6.11/qwebenginewebauthuxrequest.html
[Q02]: https://doc.qt.io/qt-6.11/qml-qtwebengine-webengineview.html
[Q03]: https://doc.qt.io/qt-6.11/qml-qtwebengine-webenginenewwindowrequest.html
[Q04]: https://doc.qt.io/qt-6.11/qwebengineurlrequestjob.html
[Q05]: https://doc.qt.io/qt-6.11/qwebengineextensionmanager.html
[Q06]: https://doc.qt.io/qt-6.11/qtwebchannel-javascript.html
[L01]: https://github.com/linux-credentials/libwebauthn/blob/master/README.md
[L02]: https://github.com/linux-credentials/credentialsd/blob/main/README.md
[W01]: https://www.w3.org/TR/webauthn-3/
[W02]: https://fidoalliance.org/specs/fido-v2.3-ps-20260226/fido-client-to-authenticator-protocol-v2.3-ps-20260226.html
[H01]: https://wiki.hypr.land/configuring/core/dispatchers/
[H02]: https://wiki.hypr.land/ipc/
