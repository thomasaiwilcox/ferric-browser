# RustBrowser development specification

Document version: 1.1.0  
Prepared: 2026-09-14  
Last updated: 2026-09-15  
Status: implementation specification; feasibility and product qualification pending  
Working product name: RustBrowser  
Primary target: Linux x86_64, native Wayland, Arch Linux, Hyprland, Omarchy  
Implementation language: Rust application logic, with Qt Quick/QML presentation,
minimal C++ interoperability, and JavaScript for page interaction  
Engine: QtWebEngine, using a qualified, security-maintained Qt 6 distribution

## Contents

1. [How an implementation agent must use this specification](#1-how-an-implementation-agent-must-use-this-specification)
2. [Product contract and scope](#2-product-contract-and-scope)
3. [User journeys and release priorities](#3-user-journeys-and-release-priorities)
4. [Architecture and dependency decisions](#4-architecture-and-dependency-decisions)
5. [Repository and build contract](#5-repository-and-build-contract)
6. [State, identity, concurrency, and ownership](#6-state-identity-concurrency-and-ownership)
7. [Engine adapter and capability contract](#7-engine-adapter-and-capability-contract)
8. [Window layout and presentation](#8-window-layout-and-presentation)
9. [Input modes and keyboard handling](#9-input-modes-and-keyboard-handling)
10. [Command language and command inventory](#10-command-language-and-command-inventory)
11. [Navigation, completion, and URL handling](#11-navigation-completion-and-url-handling)
12. [Hints, caret navigation, and page scripts](#12-hints-caret-navigation-and-page-scripts)
13. [Tabs, windows, profiles, and private browsing](#13-tabs-windows-profiles-and-private-browsing)
14. [History, bookmarks, sessions, and storage](#14-history-bookmarks-sessions-and-storage)
15. [Configuration and theme schema](#15-configuration-and-theme-schema)
16. [Permissions, authentication, and security](#16-permissions-authentication-and-security)
17. [Downloads, files, PDF, and printing](#17-downloads-files-pdf-and-printing)
18. [Media, calls, and Google compatibility](#18-media-calls-and-google-compatibility)
19. [Blocking, network policy, and compatibility quirks](#19-blocking-network-policy-and-compatibility-quirks)
20. [CLI, IPC, userscripts, and external editors](#20-cli-ipc-userscripts-and-external-editors)
21. [Wayland and desktop integration](#21-wayland-and-desktop-integration)
22. [Hyprland and Omarchy integration](#22-hyprland-and-omarchy-integration)
23. [Accessibility and international input](#23-accessibility-and-international-input)
24. [Diagnostics, failures, and recovery](#24-diagnostics-failures-and-recovery)
25. [Performance and resource budgets](#25-performance-and-resource-budgets)
26. [Test architecture and qualification matrix](#26-test-architecture-and-qualification-matrix)
27. [Milestones and implementation backlog](#27-milestones-and-implementation-backlog)
28. [Packaging, updates, and support](#28-packaging-updates-and-support)
29. [Open-source maintenance and provenance](#29-open-source-maintenance-and-provenance)
30. [Decision gates and risk register](#30-decision-gates-and-risk-register)
31. [Definition of done and traceability](#31-definition-of-done-and-traceability)
32. [Signature product direction](#32-signature-product-direction)
33. [Sources and verification notes](#33-sources-and-verification-notes)

## 1. How an implementation agent must use this specification

This document defines the intended product and the evidence needed to claim it
works. It does not claim that a Rust wrapper or a Qt API automatically provides
the required end-user behavior. Unless explicitly described as an upstream
fact with a source, behavioral rules and numerical limits below are project
decisions or proposed acceptance targets.

**SPEC-001 — Normative language.** MUST and MUST NOT are release requirements.
SHOULD permits a documented exception with rationale and tests. MAY denotes an
optional implementation. Requirements marked V1 are mandatory for the first
stable release. Requirements marked V1.1 are mandatory for the first feature
release after V1, but do not block V1. M0–M4 are development milestones, not
calendar estimates. POST denotes work outside V1 and V1.1. A feasibility gate
is not optional simply because it depends on another project.

Unless explicitly marked V1.1, POST, optional, or a target awaiting a stated
decision, each numbered requirement and its required behavior belongs to V1. A
milestone designation determines when it is implemented or proven, not whether
it is required.

**SPEC-002 — Implementation sequence.** Start with M0. Resolve and record its
engine, rendering, input, portal, and authentication findings before building
the complete UI. Continue through the dependency order in section 27. Build
working vertical slices that can be exercised in a real Wayland session.

**SPEC-003 — Evidence.** Each requirement must eventually have a traceability
entry linking implementation, automated tests where suitable, manual evidence
where necessary, milestone, and status. Accepted statuses are `not-started`,
`in-progress`, `verified`, and `blocked`. `blocked` records a concrete issue and
its owner; it does not count as implemented. Feature flags, stubs, mocks, and a
compiling application do not satisfy a user-facing requirement by themselves.

**SPEC-004 — Changes to the specification.** Resolve small implementation
details locally. Record material changes in an architecture decision record
(ADR), update this specification and its tests, and explain the impact. An
agent must not silently replace QtWebEngine, drop Google qualification, remove
native Wayland, or weaken a security boundary to pass a milestone. Those are
product decisions requiring the project owner's direction.

**SPEC-005 — Work boundaries.** Development belongs in the project repository.
Use disposable browser profiles for testing. Never use a developer's normal
browser cookies, account, configuration, or passwords as an automated test
fixture. Machine configuration changes, installing dependencies, publication,
and external account use follow the actual environment's permissions and
instructions. This document grants no additional authority over the machine.

**SPEC-006 — Honest completion.** If live Google tests require an authorized
test account or a physical security key is unavailable, finish independent
work and mark that qualification pending. Never manufacture a passing result.
Do not call a release daily-driver ready while a mandatory qualification gate
remains pending.

The examples in this document are normative target interfaces. The repository
initially contains documentation only; build commands become runnable as the
corresponding milestone is implemented.

## 2. Product contract and scope

RustBrowser is a browser for people who prefer commands, keyboard navigation,
tiling compositors, plain configuration files, and small external tools. It
must also support ordinary mouse interactions and complex web applications.
Its value is predictable control over everyday browsing with reliable modern
web compatibility.

**PROD-001 — Core promise.** A user can sign in to their work services, edit
documents, join a video call, share a screen, navigate by hints, manage tabs and
sessions, and send content to external tools without switching browsers for a
routine supported workflow.

**PROD-002 — Language boundary.** Browser policy, state transitions, commands,
configuration, persistence, integration logic, and completion MUST be Rust.
QML may describe views, bindings, accessibility properties, and minimal event
forwarding. Small C++ adapters may express public Qt APIs that the bridge does
not expose. Page interaction may require JavaScript. Generated C++ and Qt's
existing code are expected; a literal all-Rust rendering stack is not the goal.

**PROD-003 — Desktop focus.** Native Wayland on Arch/Hyprland/Omarchy is the
primary supported experience. The browser core MUST run without Omarchy or
Hyprland present. Sway is the secondary qualification compositor. Plasma and
GNOME receive compatibility smoke tests where infrastructure permits, without
the same initial support promise. X11, Windows, and macOS are outside V1.

**PROD-004 — Simplicity.** Use one engine, one primary presentation toolkit,
one command registry, and one documented source of truth for each persistent
data category. Prefer upstream public APIs and a small number of maintained
dependencies. No private Qt APIs, custom compositor, browser-engine fork,
cloud synchronization service, or native plugin ABI in V1.

**PROD-005 — qutebrowser relationship.** Reproduce its useful interaction
concepts: modal input, hints, commands, quickmarks, completion, per-site
configuration, sessions, and Unix integration. Maintain an explicit parity
matrix rather than promising all historical settings or byte-for-byte command
compatibility. The configuration language will be TOML, not Python.

**PROD-006 — Robustness.** A single tab failure must not intentionally take
down the application. Durable browser metadata must survive interruption.
Unrecognized engine capabilities must produce accurate errors. The browser
must preserve Chromium's security mechanisms and the compositor's authority.

**PROD-007 — Compatibility defaults.** JavaScript, normal cookies, storage,
WebRTC, and hardware acceleration are enabled using qualified engine defaults.
No broad fingerprinting changes, global user-agent spoofing, or aggressive
tab suspension are enabled by default. Privacy controls remain available and
explain their compatibility cost.

**PROD-008 — Resource expectations.** Rust should make the application layer
predictable and testable. It does not make Chromium pages inherently small or
guarantee faster page rendering than qutebrowser. Performance claims require
measurements that include renderer and GPU subprocesses.

**PROD-009 — Signature promise.** RustBrowser is a keyboard-driven browser that
understands profiles, tasks, windows, and Wayland workspaces as one commandable
system—and explains exactly what it is doing when the modern web misbehaves.
The product must make this promise concrete through contexts, a universal
switcher, inspectable site behavior, typed actions, discoverable bindings, and
explicit clean-link tools. These are product identity, not ornamental polish.

## 3. User journeys and release priorities

| Journey | Required outcome | First evidence | Stable gate |
| --- | --- | --- | --- |
| Everyday navigation | Open URL/search, follow hint, back/forward, close and undo tab | M1 | V1 |
| Keyboard editing | Type in Gmail/Docs, use IME and shortcuts, return to browser mode reliably | M0/M1 | V1 |
| Work profile | Work and personal profiles keep cookies, permissions, history, and downloads separate | M2 | V1 |
| Google Meet | Join, choose devices, toggle camera/mic, share screen/window, stop sharing | M0 | V1 on qualified baseline |
| Session recovery | Kill browser, restart, recover ordinary tabs without replaying a form submission | M2 | V1 |
| Unix workflow | Hint link to mpv; edit a textarea externally; query tabs through IPC | M2 | V1 |
| Omarchy theme switch | Browser chrome updates without restart or page reload | M4 | V1 |
| Hyprland workspace use | Open/focus browser windows with stable identification and optional placement | M4 | V1 |
| Task context | Enter a named task environment that selects a profile, window/session set, appearance, and optional workspace without weakening profile isolation | M1/M4 | V1 |
| Universal switcher | Find and act on any eligible tab, window, context, command, history item, mark, session, download, or recently closed item | M1/M2 | V1 |
| Site explanation | Inspect effective policy and diagnose a broken site through reversible, bounded experiments | M2/M3 | V1 |
| Clean link sharing | Preview, explain, copy, or explicitly open a URL with versioned tracking parameters removed | M1/M2 | V1 |
| Keyboard learning | Discover valid continuations and explain active or conflicting bindings from generated registry data | M1 | V1 |
| Journey recovery | Inspect and reopen branches in the relationship graph of ordinary navigation | After V1 | V1.1 |
| Ephemeral isolation | Open an isolated in-memory profile that disappears with its final window | After V1 | V1.1 |
| Failure diagnosis | Explain missing portal, unsupported codec, renderer crash, or old engine | M0 onward | V1 |
| Accessibility | Reach and identify chrome controls using keyboard and assistive technology | M1 onward | V1 |

The release priorities are:

1. Safe engine embedding, input correctness, account compatibility, and native
   desktop interactions.
2. Essential browsing and recovery.
3. Complete V1 command workflow, signature navigation tools, and extension surface.
4. Desktop polish, packaging, documentation, and repeatable qualification.

Full Chrome WebExtension support, Chrome Sync, built-in password vaults,
mobile platforms, automatic website automation, browser-owned split-view
tiling, and compositor-independent workspace assignment are POST. Hardware-
backed WebAuthn is a V1 qualification target; synced passkeys and every
authenticator transport are not implied. The journey graph and ephemeral
profiles are the defined V1.1 scope unless an accepted scope decision changes
their release placement.

## 4. Architecture and dependency decisions

### 4.1 Structural architecture

```text
CLI / key input / native UI / local IPC
                  |
                  v
      Rust command registry + dispatcher
                  |
                  v
       Rust application state + effects
          |          |           |
          v          v           v
    persistence   desktop     engine adapter
      worker      services        |
                                  v
                  CXX-Qt / narrow C++ adapters
                                  |
                                  v
                 Qt Quick window + WebEngineView
                                  |
                                  v
             QtWebEngine renderer / GPU / services
```

**ARCH-001 — Window ownership.** Qt Quick owns the Wayland windows and web
surfaces. Browser chrome and web content share that composition path. Do not
embed a foreign Qt window in winit, GTK, egui, or another toolkit, or begin with
offscreen WebEngine textures. This is a project choice to keep graphics and
input ownership tractable. Qt documents Quick-based embedding and separate
rendering processes. [Qt architecture](https://doc.qt.io/qt-6/qtwebengine-overview.html)

**ARCH-002 — Baseline.** Begin M0 against the publicly documented Qt 6.11.x
family available at specification time. Pin the actual qualified patch,
build configuration, Rust toolchain, and bridge release after testing. If work
begins later, choose the then-maintained Qt family through ADR-001 instead of
blindly installing an obsolete pin. V1 supports one qualified Qt minor family
plus an explicitly tested transition candidate. An API version floor alone is
not a security-support commitment.

**ARCH-003 — Bridge choice.** CXX-Qt is the first choice, with `cxx` for small
unsupported surface areas. Its generated Qt objects support a Rust-owned
application model. Pin bridge dependencies in Cargo.lock. A focused bridge
spike must prove lifetimes, asynchronous callbacks, QML models, and release
packaging before this choice is considered qualified.
[CXX-Qt introduction](https://kdab.github.io/cxx-qt/book/)

**ARCH-004 — Build choice.** Start Cargo-first using CXX-Qt's Cargo integration.
System Qt is discovered during build. If WebEngine/QML deployment requires
CMake, keep a single documented entry point through `cargo xtask` and record
the reason. Do not maintain unrelated Cargo-only and CMake-only products.
[Cargo integration](https://kdab.github.io/cxx-qt/book/getting-started/4-cargo-executable.html)

**ARCH-005 — Core independence.** The core crate MUST build and test without Qt,
a display server, D-Bus, or network access. It accepts typed events and emits
typed effects. It must not contain QObjects, QUrls, raw pointers, runtime
environment lookups, shell execution, or SQL connections.

**ARCH-006 — Browser engine ownership.** QtWebEngine owns HTML/CSS/JavaScript,
TLS, normal network requests, cookie and website storage formats, service
workers, engine navigation history, sandboxing, and renderer processes. Rust
uses exposed APIs to express browser policy; it must not recreate TLS,
intercept HTTPS through a proxy, or manipulate Chromium databases directly.

**ARCH-007 — Shared intent model.** Commands, typed actions, UI controls, hints,
the universal switcher, IPC, and userscripts converge on captured core subjects
and the same reducer/effect paths. Presentation layers may filter availability
but must not reimplement validation, confirmation, privacy, profile/context, or
stale-target rules. Context is task metadata above `ProfileId`; it is never
translated into a new engine-security primitive.

### 4.2 Recommended dependency roles

These are candidate choices, not unverified promises that particular crate
versions expose all required functionality. Resolve exact versions in M0 and
record maintenance status, licenses, and supported Rust versions.

| Role | Preferred starting point | Constraint |
| --- | --- | --- |
| CLI | `clap` | Generate completions from command schema where possible |
| Serialization | `serde`, `serde_json`, `toml` | Size/depth limits for untrusted input |
| SQLite | `rusqlite` | Single owning worker; explicit migrations |
| Rust/Qt | `cxx-qt`, `cxx`, supporting CXX-Qt crates | Narrow FFI modules; public Qt APIs |
| Logs | `tracing`, subscriber with field redaction | No raw page/credential content by default |
| Errors | `thiserror` for public enums | User errors are structured and actionable |
| URL values | `url` plus a tested Qt conversion boundary | Security origin comparison must agree with engine |
| IDs | UUIDs for persistent identities, generational runtime IDs | Never use vector indices as identities |
| D-Bus | `zbus`, optionally `ashpd` for supported portal requests | Avoid duplicate capture sessions |
| File watching | `notify` | Handle directory replacement and symlinks |
| Blocking | maintained `adblock` implementation from Brave's Rust project | Verify API/license and rule support in M2 |
| Worker execution | bounded threads/channels first | Introduce Tokio only if async I/O needs justify it |
| Property/fuzz tests | `proptest`, cargo-fuzz/libFuzzer | Parsers and state transitions are priority |
| Test servers | small Rust HTTP/HTTPS/WebSocket fixture service | Offline and deterministic |

## 5. Repository and build contract

**BUILD-001 — Intended layout.** Establish this logical separation; avoid empty
crates solely to match a diagram. Closely related modules may start in one
crate, but the core/Qt boundary must exist from M0.

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
README.md
LICENSE                         # added after project license selection
crates/
  browser-core/                 # state, modes, commands/actions, contexts, switcher, URL/policy models
  browser-config/               # typed schema, layers, validation
  browser-storage/              # SQLite, migrations, sessions, journey graph
  browser-engine-qt/            # Qt adapter, C++ shim, bridge contracts
  browser-desktop/              # portals, D-Bus, Hyprland, theme provider
  browser-ipc/                  # framing, protocol, local client/server
  rustbrowser/                  # executable, orchestration, QML models
  xtask/                        # developer and release tooling
qml/                            # Qt Quick views and accessible controls
assets/
  page-scripts/                 # hints, focus detection, caret, extraction
  internal/                    # static help resources
  defaults/                    # generated default config and binding docs
  link-rules/                  # versioned, tested clean-link rule data
tests/
  fixtures/                    # local web sites, certificates for tests only
  integration/                 # browser adapter and persistence tests
  wayland/                     # compositor/portal harness
  compatibility/               # Google and hardware test procedures
  fuzz/                        # fuzz targets and minimized regressions
packaging/
  arch/                        # PKGBUILD, source verification, install manifest
  desktop/                     # desktop file, icons, MIME entries
  omarchy/                     # optional integration resources
docs/
  DEVELOPMENT_SPEC.md
  architecture/                # ADRs and capability mapping
  user/                        # generated commands/config plus guides
  testing/                     # evidence, matrices, reproducible setup
  requirements.csv             # machine-readable implementation traceability
```

**BUILD-002 — Developer entry points.** Once M0 scaffolding exists, these target
commands must work from the repository root:

```sh
cargo build --locked
cargo test --locked -p browser-core -p browser-config -p browser-ipc
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
cargo xtask test engine
cargo xtask test wayland
cargo xtask package arch
cargo run -p rustbrowser -- --temp-basedir
```

`xtask check` performs deterministic local checks; online dependency advisory
checks and hardware/live-account tests are separate named tasks. Runtime tests
must say when native dependencies or a compositor are missing. They must not
silently skip and return a green complete qualification report.

**BUILD-003 — Reproducibility.** Track Cargo.lock for the application. Pin Rust
in rust-toolchain.toml and declare an MSRV when V1 is published. Record exact
Qt packages/build IDs and compiler versions in CI images. No build script may
download executable code during a locked offline build. A fresh supported
machine must be buildable using documented prerequisites and one entry point.

**BUILD-004 — Build versus distribution.** Build-time Qt headers and runtime
Qt libraries must be compatible. Package WebEngine helper executables,
resources, locales, QML imports, and platform plugins using the chosen Qt
distribution layout. Test the installed package without the source/build tree
or developer-specific environment variables.

## 6. State, identity, concurrency, and ownership

### 6.1 Domain model

**STATE-001 — Identifiers.** Define opaque `WindowId`, `TabId`, `ProfileId`,
`ContextId`, `ActionId`, `RequestId`, `DownloadId`, `DocumentId`, `SessionId`,
and `JourneyNodeId` types. Runtime IDs must not be reused within an instance.
Durable IDs use UUIDs. Every asynchronous tab result carries a tab generation
and document generation so old work cannot affect a navigated or closed tab.

| Object | Essential state | Ownership |
| --- | --- | --- |
| Application | windows, loaded profiles, active/last-focused window, config revision, shutdown state | Rust main thread |
| Window | profile, ordered tab IDs, active tab, mode stack, prompts, fullscreen intent | Rust main thread |
| Tab | profile, document generation, URL/title, load/navigation state, zoom, mute/pin state, renderer status | Rust; engine facts mirrored from Qt |
| Profile | durable ID, label, privacy kind, paths, policy snapshots, engine handle | Rust lifecycle + Qt profile lifetime |
| Context | durable ID, label, exactly one profile, window/session membership, optional workspace/accent/default target, routing rules | Rust config/state; never the engine storage boundary |
| Action definition | stable ID, typed subjects/parameters, allowed sources, effect class, privacy policy, implementation target | Rust action registry |
| Command invocation | command/typed args, source, count, captured window/tab/profile/context targets, operation ID | Rust dispatcher |
| Prompt | ID, origin context, tab/document, kind, valid choices, expiry/cancel condition | Rust + retained adapter handle |
| Download | initiating profile/document, source, safe destination, progress, engine state | Rust model + Qt download object |
| Session | version, ordered windows/tabs, safe restore descriptors, revision | Rust storage worker |
| Journey node/edge | profile-local safe URL/title, timestamp, relation kind, source/target node IDs | Rust storage worker; V1.1 and memory-only when private |

**STATE-002 — Event/effect separation.** Use a reducer-like boundary:
`reduce(state, event) -> effects`. Effects include engine actions, database
operations, desktop requests, timers, and model notifications. Side-effect
responses return as events. Pure reducers must be deterministic under an
injected clock and ID source.

**STATE-003 — Target capture.** Resolve `current` once when a command begins.
Completion callbacks, portal replies, and userscript results operate on that
captured target; changing focus must not retarget them. Operations explicitly
defined to use the latest current tab resolve again as a separate invocation.

### 6.2 Thread and callback rules

**STATE-004 — UI thread.** Qt GUI objects and their models are created, accessed,
and destroyed on the Qt GUI thread. No future may block the GUI thread waiting
for a worker that needs a Qt callback. Never use `block_on` around UI requests.

**STATE-005 — Worker ownership.** Database operations, filter compilation,
large searches, file hashing, and background metadata downloads run on bounded
workers. Start with at most four CPU workers and a dedicated SQLite owner.
Coalesce superseded completion, theme, and progress updates. Commands that
cannot be queued within limits return `E_BUSY` rather than allocate without
bound. Engine callbacks that demand an immediate answer use a precomputed
policy snapshot or a defined deny/cancel fallback.

**STATE-006 — FFI safety.** No Rust panic may unwind across FFI. Audit every
unsafe block with its ownership/thread/lifetime invariant. Recoverable errors
return typed results. A panic indicating corrupted UI state triggers controlled
termination and recovery, rather than continuing with untrusted state. Qt
request handles remain on their documented thread and are invalidated when
the owning view/profile disappears.

**STATE-007 — Exact-once resolution.** Permission prompts, authentication,
file dialogs, new-window requests, and downloads have one terminal resolution.
After cancellation, duplicate or stale responses are no-ops with diagnostic
counts. A queued callback referencing a destroyed QObject must be safely
discarded. Retaining a request uses its public lifetime mechanism; no pointer
to a stack-scoped event survives its callback.

### 6.3 Lifecycle rules

**STATE-008 — Tab lifecycle.** Model these independent dimensions rather than
one overloaded enum:

- Existence: constructing, live, closing, closed.
- Loading: idle, provisional, committed, complete, failed, cancelled.
- Renderer: healthy, terminated.
- Resource lifecycle: active, frozen, discarded, with unsupported states explicit.

Starting top-level navigation invalidates document-bound work. Same-document
navigation updates URL/history context but does not pretend a new renderer
document exists. Engine signals are authoritative about final committed URLs.

**STATE-009 — Shutdown.** Stop accepting new mutating IPC; resolve/cancel
requests; ask about active downloads/unsaved page state as required; flush Rust
metadata; destroy all views and their downloads before profiles; let Qt finish
its shutdown; mark clean exit only after required durable writes succeed.
Never kill renderer helpers first during normal exit. If shutdown exceeds ten
seconds, display its current stage and allow an explicit force-quit; preserve
the unclean marker.

**STATE-010 — Critical invariants.** At all times: every live tab belongs to one
window and one profile; a window contains only its profile's tabs; the active
tab is present or the window is in an explicit empty/closing state; a context
references exactly one profile and cannot merge profile storage; contextual
windows/tabs use that context's profile; a private profile has no durable
browser data destination; accepted requests have a valid captured target;
closed objects do not receive new effects.

## 7. Engine adapter and capability contract

**ENGINE-001 — Public APIs only.** Use `WebEngineView` for views and the public
Quick profile APIs for profiles, interception, and settings. A QML
`WebEngineView` must not be treated as exposing a public `QWebEnginePage*`.
Do not obtain private Qt objects to emulate Widgets-only APIs. Any gap is a
recorded capability limitation or a reason to reconsider the presentation
choice through an ADR.

The adapter exposes typed operations conceptually resembling:

```rust
// Interface sketch, not a promise of a particular bridge syntax.
pub struct Target {
    pub tab: TabId,
    pub generation: u64,
    pub document: DocumentId,
}

pub enum EngineEffect {
    Navigate { target: Target, url: ValidatedUrl },
    Reload { target: Target, bypass_cache: bool },
    Stop { target: Target },
    TraverseHistory { target: Target, offset: i32 },
    RunPageOperation { target: Target, request: RequestId, op: PageOperation },
    ResolvePermission { request: RequestId, decision: PermissionDecision },
    SetZoom { target: Target, factor: f64 },
    CloseView { tab: TabId, generation: u64 },
}
```

Arbitrary JavaScript strings are restricted to explicit user-script/devtools
entry points. Routine page operations use structured variants and serialized
arguments. Engine callbacks contain facts; they cannot submit arbitrary shell
commands or mutate configuration.

### 7.1 Required adapter surfaces

| Capability | Adapter responsibility | Evidence |
| --- | --- | --- |
| Navigation | URL changes, load errors, redirects, history traversal, cancel | Local redirect/POST/SPAs fixtures |
| Popup creation | Accept engine new-window request into correct profile and destination | `window.open`, opener and OAuth fixtures |
| Profiles | Explicit paths and privacy mode before navigation | Cookie/storage isolation tests |
| Input | Forward or consume complete native event pairs; IME composition | Wayland key and input method tests |
| Page operations | Async, frame-aware, isolated-world execution | Stale callback/frame/navigation tests |
| Permissions | Retain request, expose origin/type, resolve once | Allow/deny/revoke/cancel fixture |
| Web authentication | Native UX request states and cancellation | Physical FIDO2 test plus local mock flow |
| File operations | File chooser, downloads, save/print/PDF | Unicode paths, cancellation, disk full |
| Media | Playback state, fullscreen, capture requests, lifecycle eligibility | Call and screen-share qualification |
| Requests | Fast profile request policy and blocking | Main/subframe/resource fixtures |
| Diagnostics | Engine versions, GPU/sandbox/capability state | Installed package report |
| Failure | Renderer termination, profile loss, clean shutdown | Fault injection/soak tests |

The upstream Quick API exposes the principal navigation, popup, permission,
desktop capture, and authentication signals; actual integration behavior must
be proven against the pinned build.
[WebEngineView API](https://doc.qt.io/qt-6/qml-qtwebengine-webengineview.html)

**ENGINE-002 — Capability matrix.** Maintain `docs/architecture/capabilities.md`
with feature, exact public API, minimum API version, compiled support,
platform prerequisites, runtime probe, tested package version, limitation,
test ID, and fallback. Include at least WebAuthn transports, screen versus
window capture, system audio sharing, codecs, DRM, hardware decode, PDF,
printing, notifications, push, spellcheck, primary selection, and per-site
settings.

**ENGINE-003 — Request interception.** Interception executes synchronously and
must consult only bounded in-memory data. Do not call profile/UI APIs, query
SQLite, fetch lists, wait on the UI thread, or execute scripts from it. Swap a
precompiled immutable policy snapshot safely. Confirm the selected filter
engine's thread-safety rather than marking it `Send`/`Sync` unsafely. The
adapter explicitly owns the interceptor until it is detached and all relevant
requests are quiescent.
[Interceptor API](https://doc.qt.io/qt-6/qwebengineurlrequestinterceptor.html)

**ENGINE-004 — Security defaults.** Keep Chromium sandboxing, site isolation,
certificate verification, mixed-content policy, and same-origin protections
enabled. Reject production startup as root or with known disabling flags such
as `--no-sandbox`, `--single-process`, and `--disable-web-security`. Diagnostic
test overrides require a separate development build and conspicuous output;
they never become an installation fix or release-test pass.

**ENGINE-005 — Health reporting.** Capability probes report `available`,
`unavailable`, `not-tested`, or `unknown`, with reasons. Do not infer that GPU
decoding works merely because GPU compositing works, or that FIDO2 works
because a WebAuthn API is present. Missing mandatory capabilities fail the
qualification gate rather than silently downgrade the requirement.

## 8. Window layout and presentation

**UI-001 — Default layout.** Web content fills the available window. A compact
bottom status bar remains visible by default. A top tab strip appears with two
or more tabs. The command line replaces the status text while active;
completion expands upward without resizing the web page on every keystroke.
Transient messages must not repeatedly change viewport size or scroll position.

**UI-002 — Status.** Show mode, distinct profile and context labels, committed
URL/origin, connection state, loading progress, tab position, audible/mute
state, active Site Doctor experiment, capture, downloads, and pending
permissions. Omit inactive fields. Private browsing—and V1.1 ephemeral
browsing—has a textual indicator, not color alone. Distinguish HTTPS transport security from website
trust; a padlock is not an endorsement.

**UI-003 — URL display.** Origin receives priority when space is limited.
Elide the path before the host. Render untrusted titles and URLs as plain text,
with bidi/control characters escaped or visibly isolated. Credentials are
always removed from displayed URLs. Suspicious IDN cases display the ASCII
hostname; accessible text preserves the full safe origin.

**UI-004 — Prompts.** Security prompts are trusted native browser chrome. Show
requesting origin, operation, scope, and duration. New prompts never interpret
the keypress that opened them as consent. Enter does not default to permission
grant; Escape cancels/denies. Keyboard focus and the original page context are
restored only if still valid. Background tabs show a pending badge rather than
steal focus. Limit one interactive modal prompt per window with a bounded
queue, and group identical origin/type requests.

The initial prompt queue limit is eight requests per window. Cancel excess
page-generated prompts with a visible grouped notification and a 30-second
origin-specific cooldown. Do not discard a pending file overwrite or browser
shutdown decision to make room for a page alert.

**UI-005 — Page dialogs.** JavaScript alert/confirm/prompt and HTTP auth are
visually distinct from browser commands and include the requesting origin.
Rate-limit repeated page dialogs and offer suppression for that document.
Closing a tab cancels its dialogs. Page text may not choose iconography or
layout that impersonates a browser permission prompt.

**UI-006 — Theme and dimensions.** Use semantic color tokens and logical Qt
units. Default browser font is the system monospace font with system fallback;
page fonts follow WebEngine settings. Status height follows font metrics and
padding rather than a fixed physical pixel count. Chrome opacity is 1.0;
forced darkening of websites is off. Respect reduced motion. A floating-window
fallback must retain normal move/resize affordances when a compositor does
not supply the expected decoration behavior.

**UI-007 — Internal surfaces.** Help/binding map, universal switcher, Site
Ledger/Doctor, contexts, settings, tabs, history, downloads, permissions, and
diagnostics—and in V1.1 the journey graph—are accessible native surfaces or strictly
read-only internal documents. The blank/start page is local and issues no
network requests. Browser management actions are routed through native models,
not a privileged WebChannel shared with arbitrary web pages.

## 9. Input modes and keyboard handling

### 9.1 Modes

| Mode | Entry | Key behavior | Exit |
| --- | --- | --- | --- |
| Normal | Startup/default; explicit escape | Browser bindings consume matched keys | `i`, `:`, `/`, `f`, `v`, pass-through binding |
| Insert | User focuses an editable element, a hint selects one, or `i` | Page receives ordinary keys, shortcuts, and IME | Escape or explicit normal-mode command |
| Command | `:` | Native editable command line and completion | Enter executes; Escape cancels |
| Search | `/` or `?` | Incremental engine text search | Enter retains match; Escape restores prior state |
| Hint | `f` or hint command | Label selection/filtering, no text into page | Selection/cancel/navigation |
| Caret | `v` | Text movement/selection commands | Escape returns normal |
| Pass-through | `Ctrl-v` from normal | Every key goes to page except reserved escape chord | `Ctrl-Shift-Escape` |
| Prompt | Browser request | Prompt-specific keys; no page key forwarding | Answer/cancel/invalidated request |

**INPUT-001 — Mode ownership.** Modes are per-window and remember the current
tab context. Prompt/search/command/hint states are transient overlays on a base
normal/insert/pass-through state. Leaving an overlay restores a still-valid
base state; never blindly enable insert mode for a different document.

**INPUT-002 — Native events.** Use Qt's event pipeline and actual modifier
state. Do not use global key grabs or compositor-level key emulation. A handled
keypress must not leak its release into a different target or leave modifiers
stuck. Handle autorepeat, dead keys, AltGr, keyboard layout changes, and IME
composition. IME preedit text is never interpreted as normal-mode commands.
Physical/scancode bindings are optional; logical key bindings are the default.

**INPUT-003 — Editable focus.** User-initiated focus of input, textarea, or
contenteditable normally enters insert mode. Page autofocus and script-driven
focus MUST NOT change browser mode by themselves. Use a small isolated
focus-observation script and native focus signals, including frames and open
shadow roots, and require a recent trusted pointer or keyboard focus gesture
before treating observed editable focus as user initiated. Focus events are
untrusted hints about page state, not authority to execute commands. Sites such
as Docs with unconventional editors can use an explicit
`input.entry_mode = "insert"` site rule or pass-through mode. Normal-mode Escape
must not immediately be undone by a repeated stale focus event.

**INPUT-004 — Escape guarantees.** `Ctrl-Shift-Escape` returns to normal mode
from page-focused modes and cancels pending key chains. In browser prompts it
acts as cancel where cancellation is valid. It is reserved in V1 and cannot be
unbound. In fullscreen it first leaves page fullscreen and releases pointer
lock, then returns to normal. Test IME, fullscreen, and captured pointer cases.

**INPUT-005 — Chains and counts.** Bindings form a trie. Digits `1`–`9` begin
counts in normal mode; subsequent digits include `0`. Standalone `0` remains
a command key. Counts default to 1, are capped at 9,999, and obey lower
command-specific limits. Only commands declaring count support accept counts.
Ambiguous bindings wait 1,000 ms by default; an exact leaf runs immediately.
On timeout, an exact-prefix command runs, otherwise the incomplete chain is
discarded. An unmatched normal-mode key is consumed with optional feedback;
it does not type unexpected text into the page.

**INPUT-006 — Repeat and macros.** `.` repeats the last eligible browser command
with resolved command arguments but a newly resolved current target. It never
repeats passwords, permission grants, confirmation responses, arbitrary page
typing, or file overwrite consent. Macro recording stores eligible command
invocations, not raw keystrokes. Maximum nested macro depth is 8; maximum total
expanded commands per invocation is 1,000. Replay stops on error or explicit
cancel. History/repeat redact sensitive arguments.

**DISC-001 — Generated truth.** Binding discovery is generated from the merged,
validated binding trie and command registry. It shows the current mode,
captured count, typed prefix, valid continuations, command descriptions, and
source layer. It must never document a default that an active profile or user
override has replaced.

**DISC-002 — Prefix overlay.** When a valid incomplete keychain remains after
the configured delay, show a compact non-modal overlay of its continuations.
The overlay does not steal focus, reset the timeout, send keys to the page, or
change which command wins. Escape/cancellation and exact-leaf behavior remain
those of INPUT-004 and INPUT-005.

**DISC-003 — Explanation.** `binding-explain` reports tokenization, current and
requested mode, count eligibility, exact/prefix resolution, timeout behavior,
winning binding, shadowed layers, conflicts, and reserved-key constraints.
For a site-specific input-mode conflict it also reports the matching rule and
the page/editor focus facts available to the browser.

**DISC-004 — Learning mode.** Learning mode adds short post-command feedback
and more eager continuation/help affordances. It is per-window, defaults off,
never records page keystrokes, and does not alter dispatch, timing, privacy, or
the reserved escape chord. Its visible text must be accessible and suppress
optional motion when reduced motion is active. `learning-mode [on|off|toggle]`
changes only the owning window and is not a substitute for persisted config.

**DISC-005 — Searchable map.** The help surface exposes a searchable binding map
grouped by mode and action, including unbound commands and conflicts. The same
data is queryable through typed IPC without exporting private page state. The
`binding-list` command opens that surface and returns the filtered map, while
`binding-explain` returns the structured resolution for one keychain.

### 9.2 Default bindings

Bindings below are V1 defaults. A named command may have additional flags as
specified in section 10. Each help entry must show applicable modes and counts.

| Normal-mode key | Action | Normal-mode key | Action |
| --- | --- | --- | --- |
| `h` / `j` / `k` / `l` | Scroll left/down/up/right | `gg` / `G` | Top/bottom |
| `Ctrl-d` / `Ctrl-u` | Half page down/up | `H` / `L` | Back/forward |
| `o` / `O` | Prefill open in current/new tab | `go` / `gO` | Edit current URL in current/new tab |
| `J` / `K` | Next/previous tab | `T` / `gt` | Open tab selector |
| `d` / `u` | Close/undo tab | `r` / `R` | Reload/reload bypass cache |
| `f` / `F` | Hint current/new foreground tab | `;b` / `;f` / `;r` | Hint background/foreground/rapid background tab |
| `yy` / `yt` | Copy URL/title | `pp` / `Pp` | Open clipboard current/new tab (`pP` / `PP` use primary selection) |
| `/` / `?` | Search forward/backward | `n` / `N` | Next/previous match |
| `+` / `-` / `=` | Zoom in/out/reset | `m` / `b` | Save/open quickmark prompt |
| `i` / `v` | Insert/caret | `Ctrl-v` | Pass-through |
| `:` | Command line | `.` | Repeat eligible command |
| `Ctrl-p` | Toggle current-tab pin | `Ctrl-Space` | Universal switcher |
| `M` / `gb` / `gf` | Add bookmark/open bookmark/open source | `gC` / `g?` | Clone tab/searchable binding map |
| `gJ` / `gK` | Move current tab right/left | `Alt-1` … `Alt-9` / `g$` | Focus displayed tab 1 … 8 / last |
| `Alt-m` / `wi` | Toggle current-tab mute/devtools | `Sh` / `Sq` | Open history/bookmark list |
| `Ctrl-Alt-p` / `ZQ` | Print/quit | `;d` / `;y` | Hint link to download/copy URL |
| `q` + register | Toggle recording to register | `@` + register | Replay register |
| `Ctrl-Shift-Escape` | Guaranteed normal-mode escape | `Esc` | Cancel chain/selection/search as applicable |

Comma-prefixed bindings are reserved for user customization. Super-modified
bindings are left to the compositor. Normal mode has no single-key quit binding.
The normal-mode baseline follows qutebrowser's generated default binding map.
RustBrowser enhancements use otherwise-unclaimed keys and must not repurpose a
qutebrowser default. A qutebrowser command family that is not implemented yet
remains unbound (with its prefix reserved where practical) rather than receiving
an unrelated RustBrowser action; in particular, `t` remains available for the
site-setting toggle family. `ga`, `g0`/`g^`, F5/Ctrl-F5/F11, and the standard
Ctrl-based tab/window chords retain their qutebrowser meanings where the V1
command model supports them.
One-shot hint targets, including `tab-bg` and `yank`, do not require rapid
mode; `--rapid` controls whether the hint session continues after a successful
action.
Tab/Shift-Tab in command mode traverse completion; Ctrl-n/Ctrl-p do likewise.
Insert and pass-through behavior must preserve common web Ctrl-c/v/x/z/a/f
shortcuts unless a user explicitly changes those mode-specific bindings.

In caret mode, `h/j/k/l` move by character/line, `w/b` move by word, `0/$` move
to line start/end, `v` toggles selection extension, and `y` copies selection and
returns to normal mode. Search Enter retains the match and returns to the
prior base mode; normal-mode `n/N` operate on that retained search. Prompt
bindings are generated from the actual choices and never inherit normal-mode
macros or repeat handlers.

## 10. Command language and command inventory

**CMD-001 — Registry.** Define commands once with name, aliases, typed arguments,
flags, scope, mode availability, count behavior, sensitivity, effect class,
completion provider, description, and examples. Generate help, CLI command
completion, binding validation, and protocol schema from this registry.

**CMD-002 — Grammar.** Commands use Unicode text with ASCII command names,
whitespace-separated arguments, single/double quoted strings, and explicit
backslash escaping. `;;` separates commands only when outside quotes. No shell
expansion, pipes, command substitution, environment interpolation, or globbing
occurs. `--` ends flag parsing. A leading `:` is accepted by interactive command
input and the `command` CLI only, never interpreted in a URL argument.

Parse the entire chain before executing anything. Execute sequentially and
stop on the first error; earlier completed commands are not rolled back.
Aliases expand parsed tokens with cycle detection and depth 8. Placeholders
are substituted into typed argument values after parsing, never reparsed as
commands. A URL containing `;;`, quotes, `$()`, or backticks remains data.

An `<input...>` tail joins parsed tokens with a single space; quoting preserves
spaces inside one token. A string setting receives the decoded token directly.
Boolean and numeric settings use their typed parsers; list/table settings parse
the decoded token as a TOML value. This avoids asking users to preserve an
extra layer of TOML quotes around ordinary strings. Examples:

```text
:set ui.tabs never
:set ui.font_family "JetBrains Mono"
:set content.javascript false
:set navigation.start_pages '["rb://start", "https://example.com/"]'
```

**CMD-003 — Context and responses.** Every command captures window/tab/profile,
source (`keyboard`, `ui`, `cli`, `ipc`, `switcher`, `userscript`, `macro`), count, privacy,
and operation ID. Return structured success or error. Long operations first
return an operation handle, followed by completion/cancellation. Permission
grants, overwrite consent, profile deletion, and sensitive script access may
require interactive confirmation even when invoked from IPC.

**CMD-004 — Limits.** Maximum input is 64 KiB, 64 commands per chain, 256
arguments per command, alias depth 8. Reject NUL, disallowed control characters,
invalid escapes, integer overflow, and unknown flags before effects occur.
Preserve international URL/search text. Command parsing is fuzz-tested.

### 10.1 V1 command catalog

Angle brackets indicate required arguments; square brackets indicate optional
arguments. Alternatives separated by `|` are a single enum argument. Supporting
commands may be added, but these names and semantics form the target contract.

| Command | Contract |
| --- | --- |
| `open [--target current\|tab\|tab-bg\|window\|private-window] [--profile NAME] [--context NAME] [--ephemeral] [--clean-link] [--] <input...>` | Resolve URL/search once; select profile/context before navigation; clean only when explicit; ephemeral is V1.1; default current |
| `tab-open [--background] [--] <input...>` | Alias family for open in tab |
| `back [--count N]`, `forward [--count N]` | Traverse engine history; report boundary without error spam |
| `reload [--bypass-cache]` | Reload; retain engine form-resubmission protection |
| `stop` | Cancel current load |
| `scroll <up\|down\|left\|right> [--count N]` | Scroll page/container under documented focus rule |
| `scroll-page <up\|down> [--half] [--count N]` | Relative viewport scrolling |
| `scroll-to <top\|bottom>` | Scroll document extremity |
| `search [--backward] [--case smart\|sensitive\|insensitive] <text>` | In-page search; no network |
| `search-next [--backward] [--count N]` | Traverse retained matches |
| `zoom <in\|out\|reset\|FACTOR>` | Clamp 0.25–5.0; default 1.0 |
| `tab-close [--id ID] [--count N]` | Respect pinned tab/unsaved state; choose adjacent tab deterministically |
| `tab-undo` | Restore most recent eligible closed-tab descriptor in this profile |
| `tab-next [--count N]`, `tab-prev [--count N]` | Wrap over tab order |
| `tab-select <ID\|INDEX>` | Displayed indices are 1-based; IDs unambiguous |
| `tab-move [<INDEX>] [--context NAME]` | Reorder within window, or move live to a same-profile context window; otherwise refuse and offer explicit reopen |
| `tab-pin [on\|off\|toggle]` | Pinned tabs at front; ordinary bulk-close skips them |
| `tab-mute [on\|off\|toggle]` | Set engine audio-muted state |
| `tab-clone` | Open safe URL descriptor in same profile; no POST/body clone |
| `tab-detach` | Move live view into new same-profile window without navigation; M0 gate |
| `tab-give <WINDOW_ID>` | Move live tab only between same-profile windows |
| `window-new [--profile NAME] [--private]` | Create normal Wayland toplevel |
| `window-close` | Close with page/download checks |
| `window-focus <WINDOW_ID>` | Request activation; report compositor denial |
| `fullscreen [on\|off\|toggle]` | Request window fullscreen; coordinate web fullscreen |
| `hint [--target current\|tab\|tab-bg\|window\|yank\|clean-yank\|download\|userscript\|ephemeral] [--rapid] [--script NAME] [links\|all]` | Select validated live element; `ephemeral` is V1.1 |
| `mode-enter <normal\|insert\|caret\|passthrough>` | Explicit mode transition |
| `caret-move <left\|right\|up\|down\|word-next\|word-prev\|line-start\|line-end> [--count N]` | Move/extend caret according to selection state |
| `caret-select [on\|off\|toggle]`, `caret-yank` | Select and copy visible document text |
| `yank <url\|title\|selection> [--clean] [--primary]` | Write clipboard; preserve URL unless `--clean` explicitly previews/applies a transform |
| `url-clean [URL]`, `url-explain [URL]` | Preview cleaned URL or explain every retained/removed component without navigating |
| `paste-open [--target current\|tab] [--primary]` | Read clipboard and resolve as data |
| `bookmark-add [--title TEXT]`, `bookmark-delete <ID>` | Mutate current profile's bookmarks |
| `quickmark-add <NAME> [URL]`, `quickmark-delete <NAME>` | Named URL shortcut; explicit overwrite confirmation |
| `bookmark-list`, `quickmark-list`, `history` | Open native searchable manager |
| `history-clear [--since TIME] [--origin ORIGIN]` | Preview precise scope; confirm |
| `session-save <NAME>`, `session-load [--append] <NAME>` | Atomic snapshot; validate before replace/append |
| `session-list`, `session-delete <NAME>` | Session manager; delete requires confirmation |
| `profile-list`, `profile-create <NAME> [--ephemeral]` | Separate durable identity and storage; ephemeral form is V1.1 |
| `profile-open <NAME> [URL]` | Focus/create that profile's window |
| `profile-delete <NAME>` | Refuse loaded profile; preview and confirm exact data roots |
| `context-create <NAME> --profile NAME [--workspace SELECTOR]`, `context-list`, `context-delete <NAME>` | Create/list/delete task environments; never create or delete an implicit security profile |
| `context-enter <NAME> [URL]`, `context-save [NAME]` | Focus/restore a context or atomically capture its eligible window/session set |
| `context-route <add\|remove\|list> [PATTERN] [CONTEXT]` | Manage explicit routing rules; add previews conflicts and authentication safeguards |
| `switcher [--scope all\|tabs\|windows\|contexts\|commands\|history\|marks\|sessions\|downloads\|closed] [QUERY]` | Open the universal switcher over typed, privacy-filtered targets |
| `action <SUBJECT> <VERB> [typed arguments...]`, `action-list [SUBJECT]` | Invoke/discover a stable typed action shared by UI, hints, IPC, and userscripts |
| `site-status [--tab ID]`, `site-doctor [EXPERIMENT]`, `site-doctor-undo [ID]` | Explain effective live site state; apply or undo a bounded temporary experiment |
| `binding-explain <KEYCHAIN> [--mode MODE]`, `binding-list [--mode MODE]` | Explain generated binding resolution, prefix continuations, provenance, and conflicts |
| `learning-mode [on\|off\|toggle]` | Show delayed keychain guidance without changing command semantics |
| `journey [--current] [--search TEXT] [--expand NODE_ID]`, `journey-reopen <NODE_ID> [--target TARGET]` | V1.1 relationship graph and safe branch recovery |
| `set [--temp] [--pattern PATTERN] <KEY> <VALUE>` | Schema-typed runtime update; normal updates persist separately; unsupported site scope rejected |
| `get <KEY> [--url URL] [--explain]` | Effective value, source, scope, restart requirements |
| `unset [--temp] [--pattern PATTERN] <KEY>` | Remove matching runtime/generated layer override |
| `bind [--mode MODE] <KEYCHAIN> <COMMAND_TEXT>` | Parse/validate binding, then save generated override |
| `unbind [--mode MODE] <KEYCHAIN>` | Write explicit unbound marker in override layer |
| `config-reload`, `config-edit`, `config-check` | Reload atomically, edit trusted config, validate without GUI |
| `config-write-defaults <PATH>` | Write template; refuse overwrite without explicit confirmation |
| `theme-reload` | Refresh only appearance and provider status |
| `permissions [--origin ORIGIN]`, `permission-reset <ORIGIN> <TYPE>` | Inspect/revoke; explain active capture termination behavior |
| `site-data-clear <ORIGIN>` | Capability-checked website data deletion; see section 16 |
| `download <URL>`, `downloads` | Start download/show manager |
| `download-cancel <ID>`, `download-retry <ID>` | Cancel or retry according to engine support |
| `download-open <ID>`, `download-show <ID>` | Explicit desktop open/reveal after completion |
| `save-page <PATH>`, `print`, `print-pdf <PATH>` | Engine-backed operations; file collision policy applies |
| `spawn [--userscript NAME] [--] <program> [args...]` | Direct argv execution; userscript form described in section 20 |
| `edit-text` | Edit supported focused field externally |
| `script-run <NAME>`, `jseval [--world isolated\|page] <SCRIPT>` | Explicit trusted-user JavaScript execution |
| `devtools [--detach]`, `view-source` | Local engine tooling for current tab |
| `blocklist-update`, `blocking-toggle [--site]`, `blocking-status` | Transactional list update, precise exception, diagnostics |
| `macro-record <REGISTER>`, `macro-stop`, `macro-play <REGISTER>` | Record/replay eligible commands |
| `repeat [--count N]`, `cancel` | Repeat eligible operation/cancel pending UI work |
| `help [COMMAND\|SETTING]`, `version`, `diagnostics` | Local documentation and support report |
| `quit` | Normal coordinated application shutdown |

For `spawn`, `--userscript NAME` is an alternative to `<program>`, not an
additional required argument. CLI help must show the two forms explicitly.
`tab-close --count` is capped at 100 and confirms if closing the whole window;
mass destructive operations are not implied by a large navigation count.
For `set`, P-scoped keys target the captured profile by default; G-only keys
target the application. `--pattern` adds a site override within that profile
only when the setting supports S scope. JSON/IPC uses native typed values
instead of reparsing TOML fragments.

`open --context` resolves the context before its profile and refuses a
conflicting explicit `--profile`. `open --clean-link` shows the changed URL and
applies LINK-003's confirmation policy; ordinary navigation is never cleaned
implicitly. V1 command help labels `journey`, `--ephemeral`, and
the ephemeral hint target as V1.1/unavailable until implemented rather than
silently accepting them. The generic `action` form uses the action registry's
typed arguments; it does not accept arbitrary command text or shell syntax.

## 11. Navigation, completion, and URL handling

**NAV-001 — Resolution order.** Resolve input as follows:

1. An exact quickmark name, unless explicit URL-only mode is used.
2. A configured search keyword plus query.
3. A syntactically explicit, supported URI.
4. An explicit absolute/relative local path from trusted CLI or native input.
5. A plausible hostname/IP/host:port, using HTTPS by default except localhost
   and loopback where explicit/local HTTP behavior is documented.
6. A search query using the configured default search engine.

Default search is DuckDuckGo's HTTPS query endpoint. Search queries are percent
encoded as query values, not interpolated into arbitrary template syntax. No
remote suggestions, DNS probing to choose search versus URL, or speculative
network connections occur while typing by default. Internal single-label
hosts may be made explicit with a scheme or configured search-resolution rule.

**NAV-002 — Scheme policy.** Ordinary navigation supports HTTP, HTTPS, user
selected file URLs, and safe internal pages. `javascript:` is rejected by open
and clipboard entry points; explicit `jseval` is separate. `data:` documents
remain engine-origin-limited and cannot invoke native browser actions.
`mailto:` and similar external schemes use a native confirmation/desktop
handler. Deny unknown schemes by default and block remote navigation to
privileged internal management surfaces. Never auto-upgrade a failed HTTPS
connection to HTTP without a user decision.

**NAV-003 — URL canonicalization.** One tested conversion module handles Rust
URL values and QUrl. Preserve percent encoding and query/fragment content.
Use the engine-reported security origin for permission checks. Test IPv6,
default ports, IDN, trailing dots, escaped separators, file URLs, opaque
origins, and redirects. A normalization mismatch affecting policy denies the
permission/rule and reports a diagnostic; it cannot broaden access.

**NAV-004 — Navigation semantics.** Preserve redirect behavior and browser
history; never turn an engine popup request into a naive URL reload that loses
opener state, POST data, or OAuth flow. New explicit browser tabs use safe
navigation and the selected profile. Preserve engine restrictions around
cross-origin opener access and user gesture requirements.

**NAV-005 — Completion.** Combine commands/settings, current profile's tabs,
quickmarks, bookmarks, and history. Ranking is deterministic: exact match,
prefix match, token match, then recency/frequency, with a stable tie-breaker.
Display category, title, safe URL, and profile context. Query asynchronously;
discard older query revisions. Default result limit is 100, with paginated
manager views. Private windows do not query persistent history by default.

**NAV-006 — Failure surfaces.** Preserve the requested and final safe URL in an
error view. Distinguish DNS/network/TLS/HTTP/download/renderer failures. Retry
does not resubmit POST silently. Offline pages and service workers are left to
the engine; the browser must not force an error just because a network probe
failed. No network connectivity check is required for normal startup.

**LINK-001 — Explicit cleaning.** Link cleaning is an explicit operation on a
captured URL, available when copying, hinting, opening, or inspecting it. It is
off for ordinary navigation, redirects, form submissions, OAuth, payment, and
application callback URLs unless the user invokes the operation for that exact
URL. The original URL remains available until the action completes.

**LINK-002 — Rule format.** Cleaning rules are data with stable rule ID,
revision, source, matching hosts, removable parameter names or safe transforms,
exceptions, tests, and review metadata. Built-in and downloaded rules are
versioned and checksummed. They may remove recognized tracking query parameters
or normalize a documented wrapper URL; they may not execute code, contact a
network service, decode credentials, or rewrite arbitrary page content.

**LINK-003 — Preview and explanation.** Before navigating to a changed URL,
show original and result, identify each applied rule, and require confirmation
in V1. Copy-only operations may complete immediately but must expose the original and an
undo/re-copy action in transient chrome. `url-explain` reports retained as well
as removed components so a user can understand why a suspicious parameter was
not touched. Recognized credential/authentication values are masked in chrome
and logs; an explicit local reveal/copy action follows sensitive-output policy.

**LINK-004 — Conservative failure.** Preserve parameter order and encoding for
unmodified components. If parsing, wrapper extraction, host matching, or a rule
precondition is ambiguous, return the original URL with an explanation. Never
remove fragments or parameters merely because their names look random. Rules
for signed URLs, OAuth state, anti-CSRF values, payment links, and application-
specific deep links default to no transform.

**LINK-005 — Updates and audit.** Rule updates follow NET-002's transactional
download policy and retain the last accepted snapshot. Diagnostics report the
active revision without browsing URLs. The test corpus contains both expected
removals and high-value non-removals; a rule change cannot ship without corpus
coverage and a human-readable changelog.

## 12. Hints, caret navigation, and page scripts

**HINT-001 — Candidate collection.** Collect visible interactive elements:
links, buttons, inputs, selects, textareas, appropriate ARIA roles, and
contenteditable elements. Support frames and open shadow roots where public
engine frame APIs permit. Closed shadow roots, canvas-drawn controls, browser
PDF UI, and inaccessible frames have explicit limitations and retain normal
mouse/keyboard interaction. Do not imply cross-origin DOM access via ordinary
parent-frame JavaScript; inject through the engine's frame facilities.

**HINT-002 — Labels and geometry.** Default alphabet is `asdfghjkl`. Assign
deterministic prefix-free labels in viewport reading order. Keep labels stable
for a hint session while candidates remain valid. Coordinates must account for
page zoom, device scale, frame offsets, scrolling, and visual viewport changes.
Prefer native Qt Quick overlays for labels and hit targets. M0/M2 must validate
whether nested frame transforms can be represented correctly; unsupported
transforms are skipped explicitly rather than clicked approximately.

**HINT-003 — Validation before action.** Every candidate is tied to tab,
document, frame, and hint-session IDs. On selection, recheck attachment,
visibility, target kind, and geometry. Cancel/recollect on navigation or major
layout change. Mutation observers are throttled; never continuously scan the
whole page at animation-frame frequency. Hard cap candidates at 5,000 and
explain if narrowed collection is needed.

**HINT-004 — Activation.** Use direct navigation for a pure link-open action
when its semantics permit it. For buttons and controls, route a validated
native input action through the correct view when required. Synthetic DOM
`.click()` does not necessarily provide trusted user activation; test controls
requiring a user gesture, popup creation, file input, and fullscreen. Never
work around that distinction by disabling engine security. Hinting an input
focuses it and enters insert mode. Hint-to-download and hint-to-userscript
require explicit user action and use captured metadata.

**HINT-005 — Rapid hints.** A rapid background-tab/yank/clean-yank action can
stay in hint mode if the source document remains valid. `tab` opens the
validated link in a foreground tab, while `window` queues a new same-profile
window navigation and both end the current hint session. Foreground
navigation, downloads requiring consent, or document-changing actions end the
session. Limit background-tab creation to 20 per hint invocation unless
explicitly confirmed. Clean-yank applies the active link-cleaning rules to the
selected URL before writing the clipboard and reports whether the value
changed.

**PAGE-001 — Script boundary.** Package browser-owned scripts as immutable
resources with versions/hashes. Use an isolated script world. Expose no file,
shell, configuration, profile, or unrestricted command methods to page code.
Prefer one-shot serialized results to a persistent privileged WebChannel.
DOM data remains untrusted even in an isolated world. Validate types, depth,
sizes, IDs, and coordinates of returned data.

**PAGE-002 — Focus and selection.** Support document selection, caret movement,
word/line navigation, copy, and external editor extraction in ordinary DOM
text. Use Unicode-aware operations and browser selection APIs. Rich editors
with proprietary canvas models receive pass-through support; do not claim
generic caret integration there. Never extract password fields or hidden
credentials through generic selection/editor commands.

**PAGE-003 — Script timeouts.** Normal extraction has a 2-second application
deadline; hints target 200 ms. Cancellation drops late results and removes
overlays. A deadline does not imply JavaScript execution was terminated; if
the engine cannot terminate an operation safely, report that limitation and
provide stop/reload/tab-close recovery. Do not freeze the main thread waiting
for JavaScript.

**PAGE-004 — Greasemonkey subset.** V1 supports named local scripts with
`@match`, `@exclude`, `@run-at`, and frame selection, plus a documented subset
of APIs if implemented. Do not claim full Tampermonkey compatibility.
`@require` or remote dependencies must be downloaded explicitly, reviewed as
installed script content, and pinned; never fetched silently on page load.
Page-world injection is opt-in per script. Disable user scripts in private
profiles unless explicitly enabled there.

## 13. Tabs, windows, profiles, and private browsing

**TAB-001 — Profile consistency.** Each window belongs to one profile. All its
tabs use that profile. Opening a different profile creates/focuses its window.
Engine-initiated popups inherit the opener's profile, including private mode;
they may not switch profile through a hostname rule mid-authentication.

**TAB-002 — Ordering.** New related tabs follow their opener's group in creation
order; unrelated tabs append. Closing the active tab selects the next tab at
the same position, then the previous tab if none remains. Closing the final
tab creates a local blank tab by default. `window-close` closes the window.
Pinned tabs remain before ordinary tabs and resist accidental bulk closure.
Pin and mute actions preserve their typed state through Qt; tab mute also
updates the native WebEngine audio-muted state. `tab-next` and `tab-prev`
accept bounded typed `--count` traversal and wrap over the live tab order.

**TAB-003 — Live movement.** Moving a tab between windows of the same profile
must preserve its live browsing context, forms, and media. M0 must prove Qt
Quick reparenting/lifetime behavior and graphics correctness. If it cannot be
achieved through supported APIs, record a product decision: a separately named
`reopen-in-window` operation may be offered with explicit state-loss warning,
but it must not masquerade as `tab-detach` completion. Moving between profiles
always means a new navigation and must be explicit.

The reducer exposes a typed transfer boundary: `TransferTabOut` removes a live
tab without emitting a close/reload effect, and `TransferTabIn` restores its safe
metadata into a destination window. The primary Qt window now uses a persistent
view pool and can reparent the live `WebEngineView` into a same-profile window;
failed adoption or source completion rolls the destination reducer state back,
restores the source view, and preflights the secondary-source blank fallback
before releasing the source tab. The native smoke evidence must still cover
forms/media and graphics correctness before TAB-003 is fully closed. `tab-give`
now resolves the live window registry by owner token or core window ID and
supports primary-to-existing-secondary and secondary-origin reparenting;
forms/media evidence remains open TAB-003 work.

**TAB-004 — Popup policy.** User-initiated popups are allowed and mapped to tabs
or transient windows according to requested purpose. Non-user-initiated
popups are blocked by default with a per-site, visible control. Honor OAuth
opener and close semantics through engine requests. Do not allow a web page to
close an unrelated user-created tab or move arbitrary compositor windows.

**PROFILE-001 — Profiles.** A profile has a stable UUID, human-readable unique
name, storage root, cache root, and optional presentation overrides. Validate
names with a conservative slug and separate the display label. Never derive
filesystem paths from untrusted page text. Renaming a display label does not
move engine storage. Profiles are locked against concurrent process ownership.

**PROFILE-002 — Private sessions.** Create explicit off-the-record engine
profiles with no persistent paths, in-memory Rust history/session/permission
state, and no persistent favicon or download index. One private session may
span its own windows; a fresh private session gets a new profile. Named durable
profiles and private profiles never share cookies or website storage.

**PROFILE-003 — Private data exclusions.** Never write private URLs, titles,
form content, permission decisions, command history, page-script results, or
session snapshots to durable browser state or ordinary logs. Private tabs
must not appear in default IPC query results or desktop media metadata.
Explicit user downloads, clipboard copies, bookmarks, editor invocation, or
exports can write data outside the browser; surface the intended destination.
Private browsing is not protection against the OS, swap, same-user malware,
or the sites and networks being contacted.

**PROFILE-004 — Lifecycle.** Keep an off-the-record profile alive while its
windows/downloads/requests exist. Destroying the final private session removes
in-memory indices and transient grants. A profile with active views cannot be
deleted or moved. Reject a second independent owner with an actionable lock
error; do not remove locks solely because a stored PID appears stale.

**PROFILE-005 — Suspension.** Automatic discard is off in V1 defaults.
Optionally freeze/discard only engine-approved eligible hidden tabs, excluding
audio, capture, downloads, unsaved input where detectable, DevTools, and active
operations. Unknown eligibility means keep active. Releasing a discarded tab
may reload it; make that visible. Never suspend a Meet call to meet a memory
budget.

## 14. History, bookmarks, sessions, and storage

### 14.1 XDG layout and data ownership

**STORE-001 — Roots.** Honor XDG base directories with standard home-directory
fallbacks. Explicit `--basedir` redirects all browser-owned config, data,
state, cache, and runtime locations into that base for isolated testing.
`--temp-basedir` creates a new private-permission directory and records its
exact cleanup target. It does not read normal config unless explicitly given.

```text
$XDG_CONFIG_HOME/rustbrowser/
  config.toml                   # user-maintained, never automatically rewritten
  bindings.toml                 # optional user binding layer
  theme.toml                    # optional user appearance layer
  profiles.toml                 # optional declarative profile definitions
  contexts.toml                 # optional task contexts and routing rules
  userscripts/                  # trusted executable scripts/manifests
  greasemonkey/                  # installed local page scripts
$XDG_DATA_HOME/rustbrowser/
  profiles/<uuid>/browser.sqlite # Rust-owned metadata
  profiles/<uuid>/webengine/     # engine-owned storage, opaque to Rust
  blocklists/                   # source metadata and validated lists
$XDG_STATE_HOME/rustbrowser/
  runtime-overrides.toml         # generated :set/:bind layer, not dotfiles config
  profiles.json                 # durable profile registry, names/UUIDs only
  contexts.json                 # stable context IDs and saved membership only
  sessions/<uuid>/               # named and recovery snapshots
  logs/                         # bounded redacted logs
  migrations/                   # migration manifests, no credential copies
$XDG_CACHE_HOME/rustbrowser/
  profiles/<uuid>/webengine/     # engine cache
  blocklists/                   # compiled filter cache, rebuildable
$XDG_RUNTIME_DIR/rustbrowser/
  <instance-key>/control.sock
  <instance-key>/lock
  <instance-key>/editor/
```

Directory permissions default to 0700 and sensitive metadata files to 0600.
Cookies and browser state are excluded from dotfiles. Runtime socket directory
creation must fail if ownership/type/permissions are unsafe; do not fall back
to a predictable shared `/tmp` socket. Session snapshots contain browsing data
and deserve the same permissions as history.

**STORE-002 — Ownership.** Rust owns bookmarks, visits, quickmarks, command
history, download metadata, grants defined in section 16, and session
descriptors. Qt owns cookie/site storage, service workers, cache, and its live
history. Native engine data stays opaque and is not queried with SQL.

### 14.2 Rust metadata schema

**STORE-003 — Initial logical tables.** Use explicit migrations and foreign keys.
The following is the minimum logical schema, not a requirement to preserve
these SQL spellings forever:

| Table | Columns and constraints |
| --- | --- |
| `schema_migrations` | integer version PK, checksum, applied UTC timestamp |
| `pages` | page ID PK, normalized URL unique, safe title, first/last visit, visit count |
| `visits` | visit ID PK, page ID FK, committed time, transition kind; indexed by time/page |
| `bookmarks` | UUID PK, URL, title, folder/tag metadata, created/updated timestamps |
| `quickmarks` | name unique, URL, created/updated timestamps; names case-sensitive |
| `command_history` | ID PK, redacted eligible command text, timestamp; bounded count |
| `permission_rules` | normalized origin, permission type, decision, persistent expiry, update time; unique origin/type |
| `downloads` | UUID PK, safe source URL, destination, state, bytes, created/completed time |
| `site_preferences` | origin/pattern key, supported preference JSON, version; schema validated |
| `journey_nodes` | V1.1 node UUID PK, safe URL/title, committed time, transition kind; profile-local |
| `journey_edges` | V1.1 source/target node FKs, relation kind, created time; unique relation triple |

The database is profile-local; cross-profile queries require explicit user
selection. Permission data has one authority as defined in section 16. Do not
store raw request bodies, password fields, auth headers, clipboard history,
or page DOM snapshots in these tables.

**STORE-004 — SQLite behavior.** Enable foreign keys, WAL, a 5-second bounded
busy timeout, and `synchronous=FULL` for durable user data. Batch high-volume
history writes off the UI thread. A confirmed bookmark/permission edit must
be committed before returning durable success. Check schema version before
write. Refuse a database newer than the application understands. Run migrations
transactionally, record checksums, and retain one versioned pre-migration
backup with clear size/retention limits. Use SQLite backup APIs or a quiesced
copy that includes WAL semantics; copying the main file while writers run is
not a backup.

**STORE-005 — History policy.** Record successful committed navigations and
same-document URL changes through a deduplicated policy. Default retention is
90 days; command history is capped at 1,000 eligible entries. Never persist
HTTP authentication credentials embedded in URLs. Recognized authentication
callback URLs and sensitive command arguments are excluded from history and
session persistence; keep the necessary live URL only in engine memory.
Arbitrary URLs can contain secrets, so browsing data is never treated as
non-secret merely because known token keys were filtered.

### 14.3 Sessions and recovery

**SESSION-001 — Snapshot contract.** JSON snapshots include schema version,
session UUID/name, profile UUID, revision/time, ordered windows/tabs, selected
tab, safe restore URL, pin/mute/zoom state, and optional scroll position.
Windows record intended ordering and optional workspace hints, not a promise
to recreate absolute compositor geometry. No forms, passwords, POST bodies,
renderer memory, cookies, or private tabs are serialized.

```json
{
  "schema_version": 1,
  "session_id": "7094eaa1-4c2c-46a8-9766-ffdd4d030061",
  "name": "work",
  "profile_id": "e1b96359-f88e-441b-9221-2217bb22a7e0",
  "revision": 42,
  "saved_at": "2026-09-14T12:00:00Z",
  "windows": [
    {
      "selected_tab": "0712f447-757f-4d5b-b911-39d5b8529eaa",
      "tabs": [
        {
          "id": "0712f447-757f-4d5b-b911-39d5b8529eaa",
          "url": "https://mail.google.com/",
          "restore_method": "safe-get",
          "pinned": true,
          "muted": false,
          "zoom": 1.0
        }
      ]
    }
  ]
}
```

**SESSION-002 — Atomicity.** Write a temporary file in the same directory,
flush and fsync it, atomically rename, then fsync the containing directory.
Keep the prior validated generation. Debounce recovery saves by 500 ms and
checkpoint at least every five seconds while dirty. Tests establish the
maximum loss window and atomicity under process kill and disk failure.

**SESSION-003 — Restoration.** After an unclean exit, offer recovery without
loading every page immediately. Restore the selected eligible tab first and
keep other URLs unloaded until selected. A safe restore performs GET only.
If the original navigation depended on POST, ephemeral blob/data URLs, auth
callback state, or an unknown unsafe transition, restore a placeholder with
explicit user options rather than automatically navigate. Do not promise
full back/forward history across process restarts; live engine history remains
supported. Preserve original snapshots until a successful restore is saved.

**SESSION-004 — User actions.** Session load defaults to a preview/replace flow
with unsaved-page/download checks; `--append` adds windows within the named
profile without closing existing ones. Session delete and history clearing
show the exact scope. Closed-tab undo is bounded to 100 descriptors per
profile and subject to the same privacy and restore rules. A saved context may
reference one or more named sessions, but each session remains bound to the
context's single profile and independently validates its restore descriptors.
Context definitions and routes are configuration; `contexts.json` stores only
stable IDs and saved membership needed to avoid rewriting user-maintained TOML.

**STORE-006 — Corruption and disk full.** Preserve the original database/file,
offer read-only inspection/export or a clearly named new profile, and show the
failure. Never silently replace user data with empty state. Recovery writes
must not retry in a busy loop. Keep metadata locks until the owner is safely
closed.

## 15. Configuration and theme schema

### 15.1 Resolution and lifecycle

**CONFIG-001 — Typed schema.** Every setting declares key, type, default,
allowed range/values, supported scope, apply time, engine prerequisite,
sensitivity, and documentation. Unknown keys, invalid enum values, and
unsupported scopes are errors. Deprecated names have explicit migrations and
warnings for one supported migration window. Configuration is declarative;
loading TOML never executes code.

**CONFIG-002 — Precedence.** Apply these layers from lowest to highest:

1. Compiled defaults.
2. Desktop theme provider values, for appearance keys only.
3. User files: includes in listed order, then config.toml; bindings.toml,
   theme.toml, profiles.toml, and contexts.toml apply to their own domains
   after config.toml.
4. Explicit profile overrides.
5. Matching site rules, ordered by ascending integer `priority`, then their
   appearance in the merged source. Later rules win per key. Default priority
   is 0. There is no hidden specificity algorithm.
6. Generated interactive overrides from `:set`/`:bind`, with the same explicit
   scope/rule semantics.
7. Process command-line `--set` overrides.
8. Temporary runtime overrides from `:set --temp`.

`get --explain` must reveal every contributing layer and the winner. A changed
lower-priority value may remain shadowed; show that plainly. `unset` removes
the generated value, exposing the lower layer. `unset --temp` removes the
temporary value. Private-window interactive overrides remain in memory and
must not change durable normal-profile behavior.

**CONFIG-003 — Reload transaction.** Read files with size limits, parse all
includes, validate schema and bindings, compile patterns, and build a candidate
configuration. Commit one revision only when the candidate is valid. Otherwise
keep the last known good configuration and show file/line/key diagnostics.
Apply live keys immediately. For reload-required or restart-required keys,
report pending changes without changing unrelated live values. Do not reload
pages or restart capture because a theme changed.

The command surface exposes this lifecycle explicitly: `config-check` validates
the configured file and adjacent contexts/profiles files without applying a
candidate; `config-reload` requests the same debounced transactional watcher
path and reports rejection without replacing the last known good revision;
`config-write-defaults <PATH>` creates a mode-restricted starter file only when
the destination does not already exist; and `theme-reload` refreshes the
appearance palette and provider status without reloading pages or restarting
capture.

**CONFIG-004 — Includes and file watching.** Includes are trusted local paths
relative to the including file, maximum depth 8 and total size 2 MiB. Detect
cycles and repeat imports. No remote includes or environment expansion. Watch
parent directories as well as files so atomic replacement works. Debounce
events by 150 ms, and wait for a coherent candidate on partial writes.

**CONFIG-005 — UI persistence.** Save interactive settings in
runtime-overrides.toml atomically; never rewrite user config. Structured data
stores unbound bindings explicitly rather than deleting lower-layer values.
Offer `config-export` as an optional convenience to produce a reviewed merged
file, not as an automatic replacement. Settings that select executable tools
must be visibly classified as trusted local code configuration.

### 15.2 Canonical starter configuration

This is a valid starter document. Omitted keys use the registry defaults. Every
key below must be accepted by the V1 schema. Values represent product defaults
unless commented as a customization example.

```toml
schema_version = 1
include = []

[ui]
statusbar = "always"
tabs = "multiple"
tab_position = "top"
font_family = "monospace"
font_size_pt = 10.0
reduced_motion = "system"

[theme]
source = "auto"
path = ""

[input]
entry_mode = "normal"
auto_insert = true
keychain_timeout_ms = 1000
count_limit = 9999

[discovery]
keychain_overlay = true
keychain_overlay_delay_ms = 350
learning_mode = false

[navigation]
default_search = "ddg"
start_pages = ["rb://start"]
new_tab = "rb://blank"
external_links = "tab"

[links.cleaning]
enabled = true
rules = "builtin"
confirm_navigation = true
# Optional update source; both fields are required together. When configured,
# explicit updates and at-most-daily normal-profile refreshes are allowed.
# update_source = "https://updates.example.test/rustbrowser-links.toml"
# update_sha256 = "<64 lowercase hexadecimal characters>"

[search_engines]
ddg = "https://duckduckgo.com/?q={query}"
g = "https://www.google.com/search?q={query}"

[tabs]
last_close = "blank"
related_position = "after-opener"
undo_limit = 100
auto_discard = false

[session]
restore = "ask-after-crash"
lazy_restore = true
checkpoint_seconds = 5

[switcher]
max_results = 100
include_private = false

[history]
retention_days = 90
command_limit = 1000

[content]
javascript = true
images = true
force_dark = false
autoplay = "engine-default"
zoom = 1.0

[privacy]
remote_suggestions = false
push_service = false
private_history_suggestions = false

[permissions]
camera = "ask"
microphone = "ask"
screen_capture = "ask"
notifications = "ask"
geolocation = "ask"
clipboard = "ask"
local_fonts = "ask"

[blocking]
enabled = true
network_filtering = true
cosmetic_filtering = false
update_interval_hours = 24
lists = ["easylist", "easyprivacy"]

[downloads]
directory = "xdg-downloads"
ask_destination = true
collision = "ask"
open_when_complete = false

[spellcheck]
enabled = true
languages = ["system"]

[desktop]
portals = "auto"
notifications = true
media_keys = true

[hyprland]
enabled = "auto"
workspace_routing = false

[site_doctor]
temporary_experiments = true

[ipc]
enabled = true
private_queries = false

[logging]
level = "info"
max_file_mib = 5
retained_files = 3

[bindings.normal]
",v" = "spawn -- mpv {url}"
",V" = "hint --target userscript --script video links"

[[site_rules]]
id = "google-docs-input"
pattern = "https://docs.google.com/*"
priority = 10
[site_rules.set]
"input.entry_mode" = "insert"
```

The comma bindings and Docs site rule are examples rather than compulsory
defaults. The named `video` userscript must be installed/configured before its
binding can execute; schema validation permits unresolved optional script
names with a warning, while invocation returns `E_NOT_FOUND`.

### 15.3 Profile, context, permission, and tool configuration

`profiles.toml` uses this structure. The persistent registry assigns UUIDs on
first creation and keeps them stable. A declarative profile disappearing from
the file never deletes its data. At most one profile can be the default;
otherwise the builtin `default` profile is used. Duplicate names and unknown
override keys are errors. Removing a profile requires the explicit delete
operation and its checks.

```toml
schema_version = 1

[[profiles]]
name = "work"
label = "Work"
default = false
overrides = { "content.zoom" = 1.0, "input.entry_mode" = "normal" }
```

`contexts.toml` uses the following structure. A context refers to an existing
named profile; it never creates, aliases, or merges profile storage. Workspace
selectors are compositor hints, not portable geometry. A route is evaluated
only for the listed browser-controlled entry points and defaults to a prompt.
Routes cannot automatically re-home engine popups, redirects, submitted forms,
or an active authentication chain.

```toml
schema_version = 1

[[contexts]]
name = "work"
label = "Work"
profile = "work"
workspace = "3"
accent = "#7aa2f7"
default_target = "reuse-or-window"
sessions = ["work"]

[[routes]]
id = "company-work"
pattern = "https://*.company.test/*"
context = "work"
behavior = "prompt"
entry_points = ["external-open", "explicit-open"]
```

Context names use the same conservative slug/display-label separation as
profiles. `default_target` accepts `reuse`, `reuse-or-window`, or `window`.
`behavior` is `prompt` in V1; `suggest` may annotate the switcher without an
automatic move. Duplicate patterns targeting different contexts are an error
unless explicit priorities make the result unique. `context-save` records
eligible membership and session references in generated state; it never edits
this file. A context accent is presentation-only and must retain accessible
contrast or fall back to the active theme token.

The following optional additions belong in config.toml. `permission_rules`
accept only a named normal profile and exact normalized origin. Allowed types
are the implemented permission names from section 16. Reject durable screen
capture grants and wildcard origins. A private or ephemeral profile never
inherits a normal profile's remembered/configured permission allow rule
implicitly. An empty
editor argv means unconfigured and yields a helpful error on invocation.

```toml
[tools]
editor = ["foot", "-e", "nvim", "{file}"]

[action_targets.mpv]
subject_types = ["url", "link"]
executable = "mpv"
argv = ["{url}"]
detach = true
allow_private = false

[[permission_rules]]
id = "work-gmail-notifications"
profile = "work"
origin = "https://mail.google.com"
permission = "notifications"
decision = "allow"
```

`tools.editor` is a G/P-scoped argv array, default empty, applied on the next
editor invocation. `{file}` must appear exactly once as a complete argument;
other executable/argument strings are literal. `config-edit` and `edit-text`
share this launcher but have separate file/privacy/write-back rules. The
terminal/editor names above are an explicit user example, not dependencies
assumed on every Omarchy installation.

An `action_targets.<name>` entry registers a trusted local destination for the
typed `send` action. Names use conservative slugs. `subject_types` must be a
nonempty subset implemented by the action registry; `executable` is a fixed
program/path; `argv` is a direct argument vector whose complete-argument
placeholders are declared fields for those subjects. No implicit shell,
environment expansion, or page-selected executable is allowed. `detach`
chooses the documented subprocess lifetime and `allow_private` defaults false.
This simple target form is appropriate for tools such as mpv; a program that
needs responses or several browser fields uses a userscript manifest instead.

Permission config rules use `allow`, `deny`, or `ask`. Conflicting duplicate
profile/origin/type rules are rejected. Rule removal exposes the lower-priority
remembered decision; `get --explain`/permission inspection must reveal this.
To require a fresh prompt despite a remembered decision, add an explicit `ask`
rule or reset the remembered decision.

### 15.4 Setting registry requirements

Scope codes: G = application-wide, P = profile, S = site/document. Apply times:
L = live, N = next navigation/new view, R = process restart. All numeric limits
must be validated before casting to Qt types.

| Key/group | Type and default | Scope / apply | Required behavior |
| --- | --- | --- | --- |
| `schema_version` | integer 1 | G / load | Reject newer unknown schema |
| `include` | list of local paths, empty | G / load | Merge order defined above |
| `ui.statusbar` | always/command/never, always | G,P / L | Security/capture indicators remain accessible |
| `ui.tabs` | always/multiple/switching/never, multiple | G,P / L | Hidden tab count still available |
| `ui.tab_position` | top/bottom/left/right, top | G,P / L | Side tabs need bounded width and keyboard access |
| `ui.font_family` | nonempty string, monospace | G,P / L | Safe system fallback |
| `ui.font_size_pt` | number 6–40, 10 | G,P / L | Logical-point scaling |
| `ui.reduced_motion` | system/on/off, system | G / L | Remove optional animation |
| `theme.source` | auto/builtin/file/omarchy, auto | G / L | Missing provider uses builtin fallback |
| `theme.path` | local path or empty | G / L | Data-only theme file |
| `input.entry_mode` | normal/insert/passthrough, normal | G,P,S / N | Applies at new document activation |
| `input.auto_insert` | bool true | G,P,S / L | Editable focus transition |
| `input.keychain_timeout_ms` | 100–5000, 1000 | G / L | Existing chain retains original timeout |
| `input.count_limit` | 1–9999, 9999 | G / L | Command-specific limits still apply |
| `discovery.keychain_overlay` | bool true | G,P / L | Derived continuations only; never page-controlled |
| `discovery.keychain_overlay_delay_ms` | 100–2000, 350 | G,P / L | Delay starts after an incomplete valid prefix |
| `discovery.learning_mode` | bool false | G,P / L | Adds explanations without altering dispatch |
| `navigation.default_search` | registered engine name, ddg | G,P / L | Must resolve to valid template |
| `navigation.start_pages` | list of validated URLs, local start | G,P / N | Explicit startup only |
| `navigation.new_tab` | validated URL, local blank | G,P / N | New unrelated tabs |
| `navigation.external_links` | tab/tab-bg/window, tab | G,P / L | External CLI/desktop opens |
| `links.cleaning.enabled` | bool true | G,P / L | Explicit operations only; does not mutate normal navigation |
| `links.cleaning.rules` | builtin or registered source, builtin | G,P / L | Versioned data; last-known-good update |
| `links.cleaning.confirm_navigation` | bool true | G,P / L | V1 true-only for changed navigations |
| `links.cleaning.update_source` | optional HTTPS URL, unset | G / L | Explicit update plus at-most-daily normal-profile refresh; no private-profile traffic |
| `links.cleaning.update_sha256` | 64 lowercase hex characters, unset | G / L | Required with `update_source`; candidate bytes must match exactly |
| `search_engines.*` | HTTPS template with one `{query}` | G,P / L | Encode query; no executable template |
| `tools.editor` | direct argv array, empty | G,P / L | Trusted executable configuration; `{file}` exactly once |
| `action_targets.*` | validated typed table, none | G,P / L | Fixed direct argv destination; no shell or dynamic executable |
| `tabs.last_close` | blank/window, blank | G,P / L | Close-window checks still apply |
| `tabs.related_position` | after-opener/end, after-opener | G,P / L | Deterministic ordering |
| `tabs.undo_limit` | 0–1000, 100 | G,P / L | Zero disables undo recording |
| `tabs.auto_discard` | bool false | G,P / L | Eligibility rules never bypassed |
| `session.restore` | never/ask-after-crash/always, ask-after-crash | G,P / N | Always still uses safe restore descriptors |
| `session.lazy_restore` | bool true | G,P / N | Selected tab first |
| `session.checkpoint_seconds` | 1–60, 5 | G / L | Loss window documented |
| `switcher.max_results` | 10–1000, 100 | G,P / L | Async revisions and stable deterministic ranking |
| `switcher.include_private` | bool false | G / L | Also requires explicit invocation opt-in |
| `history.retention_days` | 0–3650, 90 | G,P / L | Zero disables new durable history, not automatic deletion |
| `history.command_limit` | 0–10000, 1000 | G,P / L | Sensitive commands excluded |
| `content.javascript` | bool true | G,P,S / N | Disable scripting through supported engine settings |
| `content.images` | bool true | G,P,S / N | View-level setting, not profile-wide surprise |
| `content.force_dark` | bool false | G,P,S / N | Only advertised when supported/tested |
| `content.autoplay` | engine-default/require-gesture, engine-default | G,P,S / N | Qualified engine policy |
| `content.zoom` | number 0.25–5.0, 1.0 | G,P,S / L | Per-tab command overrides until navigation/rule change |
| `privacy.remote_suggestions` | bool false | G,P / L | V1 false-only unless provider implemented/qualified |
| `privacy.push_service` | bool false | P / R | Explicit opt-in; external service documented |
| `privacy.private_history_suggestions` | bool false | G / L | Does not persist private browsing history |
| `permissions.*` | ask/deny; exact-origin rules may allow eligible types | G,P / L | Screen capture cannot be globally allowed |
| `blocking.enabled` | bool true | G,P,S / L | Per-site bypass affects blocking only |
| `blocking.network_filtering` | bool true | G,P / L | Unsupported build cannot silently claim blocker active |
| `blocking.cosmetic_filtering` | bool false | G,P,S / L | POST unless separately qualified |
| `blocking.update_interval_hours` | 1–168, 24 | G / L | Last-known-good lists remain usable offline |
| `blocking.lists` | list of registered IDs/explicit HTTPS sources | G / L | Source changes require explicit fetch policy |
| `downloads.directory` | xdg-downloads or absolute path | G,P / L | Resolve through XDG user directories |
| `downloads.ask_destination` | bool true | G,P / L | Selection before engine accept |
| `downloads.collision` | ask/rename, ask | G,P / L | No automatic overwrite option in V1 |
| `downloads.open_when_complete` | bool false | G,P / L | V1 false-only; explicit open command available |
| `spellcheck.enabled` | bool true | P / L | Missing dictionaries reported |
| `spellcheck.languages` | list of BCP-47 tags or system | P / L | Resolve system to installed dictionaries |
| `desktop.portals` | auto/required, auto | G / R | No screenshot/capture bypass on missing portal |
| `desktop.notifications` | bool true | G,P / L | Per-site permission still required |
| `desktop.media_keys` | bool true | G / L | Advertise only actions actually supported |
| `hyprland.enabled` | auto/on/off, auto | G / L | Bounded reconnect and version probing |
| `hyprland.workspace_routing` | bool false | G / L | Explicit configured routes only |
| `site_doctor.temporary_experiments` | bool true | G,P / L | Bounded reversible experiments; no TLS/data-clearing experiment |
| `ipc.enabled` | bool true | G / R | CLI local operations still work |
| `ipc.private_queries` | bool false | G / L | Explicit query opt-in also required |
| `logging.level` | error/warn/info/debug, info | G / L | Redaction independent of level |
| `logging.max_file_mib` | 1–50, 5 | G / L | Bounded rotation |
| `logging.retained_files` | 1–10, 3 | G / L | Total default 15 MiB |
| `bindings.<mode>.*` | command string or explicit unbound marker | G,P / L | Validate grammar, reserved escape, ambiguity |

**CONFIG-006 — Site patterns.** V1 supports an explicit Chromium-style subset:
scheme `http`, `https`, or `*` (HTTP+HTTPS only); exact host, `*`, or leading
`*.` label wildcard; optional exact port; and path glob `*`. Normalize host and
default ports, match path without query/fragment, and reject ambiguous syntax.
`*.example.com` matches the apex and its subdomains, never `badexample.com`.
Do not use an unbounded regex engine for URL patterns. File/internal schemes
are outside site-pattern matching. Permission grants use exact origins rather
than this general-purpose wildcard pattern grammar.

An omitted port matches the default port for the selected scheme; an explicit
port matches that port only. A wildcard scheme expands to HTTP/default 80 and
HTTPS/default 443 when no port is given. Literal IPv6 hosts require bracket
syntax. Make this narrower contract visible in help rather than imply every
upstream match-pattern extension is implemented.

**CONFIG-007 — Honest scopes.** A setting supported only per-profile, such as
an engine-wide profile value, cannot be presented as per-site by mutating it
when tabs gain focus. This would leak behavior between tabs and requests.
Reject unsupported scope combinations and document capabilities. Global proxy
configuration is an advanced POST setting until its behavior across all
profiles is proven; per-profile proxy support must never be assumed.

### 15.5 Appearance tokens

**THEME-001 — Semantic theme.** Support `background`, `surface`, `foreground`,
`muted`, `accent`, `border`, `selection_background`, `selection_foreground`,
`error`, `warning`, `success`, `private`, and `mode_insert` colors. Colors are
`#RRGGBB` or `#RRGGBBAA`; clamp browser chrome transparency to safe readable
values and retain opaque security prompts. Builtin fallback uses background
`#1e1e2e`, foreground `#cdd6f4`, accent `#89b4fa`, error `#f38ba8`, warning
`#f9e2af`, and success `#a6e3a1`; derive remaining tokens deterministically.

Theme changes only affect browser-owned chrome by default. Do not inject CSS
into Google Docs or force dark mode because the desktop theme is dark. Keep
mode and permission state identifiable by labels and icons if colors collide.

## 16. Permissions, authentication, and security

### 16.1 Trust boundaries

**SEC-001 — Untrusted inputs.** Treat web pages, page scripts' DOM results,
download names, URLs, titles, external URI launches, blocklists, imported data,
and IPC payloads as parsed data. Local configuration and deliberately installed
external scripts are trusted user code/data, but must still be validated for
accidental error. A same-user executable can already access user files;
the local IPC protocol is not a sandbox against a fully compromised account.

**SEC-002 — No web-to-native authority.** Ordinary page JavaScript must have
no route to run commands, enumerate profiles, read arbitrary files, grant
permissions, or invoke local IPC. Isolated-world scripting is not a guarantee
that DOM content is trustworthy. Chromium's renderer sandbox remains the
primary page isolation boundary; Rust memory safety does not replace it.

**SEC-003 — Internal documents.** Register internal schemes before profile/view
creation according to public Qt requirements. Serve only fixed bundled assets
or escaped read-only data with a strict CSP, no remote resources, no traversal,
and no broad local/secure/CORS flags merely for convenience. A remote page may
not invoke a privileged action by opening `rb://...`. Dynamic management UI is
native. DevTools access is local and explicitly opened.
[Scheme API](https://doc.qt.io/qt-6/qwebengineurlscheme.html)

**SEC-004 — Logging and diagnostics.** Redact passwords, PINs, auth headers,
cookies, bearer tokens, query strings, fragments, userinfo, and raw form input.
Page console logs and full navigation traces are off by default. Turning debug
on does not disable redaction. Diagnostics show a preview before export and
never upload automatically. Private browsing data stays excluded even during
debugging unless a dedicated disposable test profile is used.

### 16.2 Permission authority and persistence

**PERM-001 — One authority.** Rust's permission service owns user decisions and
configuration rules. Configure the selected Qt Quick profile to request
decisions from the application rather than independently retain persistent
permission decisions: use the public `AskEveryTime` policy and verify its
behavior in M0. This intentionally opts into application-managed storage for
consistent profile/private/config behavior. It is not the upstream default.
Do not maintain an uncontrolled second permission store in Qt alongside Rust.
If this mapping proves incomplete, choose and document one alternate authority
before continuing; never present false effective permissions.

Qt distinguishes persistent permission types from per-request types; camera
and microphone requests are not stored like notifications. Rust's proposed
remembered rules must therefore be applied to each eligible request, not
assumed to be retained by Qt.
[Permission types](https://doc.qt.io/qt-6/qwebenginepermission.html) ·
[Quick profile policies](https://doc.qt.io/qt-6/qquickwebengineprofile.html)

**PERM-002 — Decision key.** Key decisions by profile UUID, exact normalized
requesting origin `(scheme, host, effective port)`, permission type, and lifetime.
Keep top-level origin/document/frame as request context when supplied. Never
derive the origin from the visible title or untrusted page message. Opaque or
unverifiable origins cannot receive persistent grants. A permission should not
be presented as top-level-site partitioned if the engine cannot enforce that
partition; state the actual origin scope in the prompt.

**PERM-003 — Resolution order.** First enforce non-overridable policy
constraints and request validity; then apply exact-origin user config rules;
then remembered durable decisions; then session decisions; then the global
ask/deny default. General site-rule wildcards cannot grant device access.
Persistent allow rules require an exact HTTPS origin, with loopback exceptions
only for explicit development/test use. Permission UI records decisions in
the Rust store only after user consent and successful validation.

| Permission | Default | Available affirmative lifetime | Special handling |
| --- | --- | --- | --- |
| Camera/microphone | Ask | Current document, profile session, remembered exact origin | Separate device capture indicators; revoke terminates capture or offers reload if required |
| Screen/window capture | Ask every request | This capture only | Portal chooser required; no durable allow or silent source reuse |
| Notifications | Ask | Session or remembered origin | Native notification bridge and revoke |
| Geolocation | Ask | Document/session/remembered origin where backend supports it | No approximate-location promise without implementation |
| Clipboard read/write | Ask when engine requests | Document/session/remembered origin | User keyboard copy/paste stays ordinary input; no silent clipboard polling |
| Local fonts | Ask | Session or remembered origin | Explain scope and fingerprinting exposure concisely |
| Pointer lock/fullscreen | User gesture and visible indication | Current interaction | Guaranteed escape; no global lock grant |
| External protocol handler | Ask | Remember origin+scheme if supported | Scheme allowlist; shell-free desktop launch |
| Unknown future type | Deny with explanation | None until implemented | No default-allow catch-all |

**PERM-004 — Prompt lifecycle.** A pending prompt expires on tab close, document
replacement, profile shutdown, explicit cancel, or an engine-invalid request.
Do not reuse the consent for a later navigation. Persisted allow applies only
to the exact key and not to an entire Google domain family. Screen capture
consent cannot be replayed by macros or command history.

**PERM-005 — Revocation.** Remove durable/session rules, reset any engine cache
that applies, and terminate active use through supported APIs. If the engine
requires navigation or tab close to stop a particular resource, present that
immediately and offer it. Do not display "camera stopped" merely because a
database row was removed. Test current streams, subsequent requests, and
restart behavior.

### 16.3 Authentication and website security

**AUTH-001 — Account flows.** Keep cookies and engine storage consistent across
same-profile tabs/popups. Handle account chooser pages, redirect chains,
multi-factor prompts, and popup closure correctly. Do not automate CAPTCHAs,
store credentials, or evade account-security restrictions. Google web sign-in
qualification is distinct from arbitrary third-party OAuth provider policy
for embedded browsers.

**AUTH-002 — HTTP/proxy/client certificate requests.** Use native origin/host
labeled prompts. Do not log responses. Credentials remain in short-lived
memory/engine state, not browser SQL or config. Client certificate selection
uses engine-offered certificate identities and cancels safely if unavailable.
Never export private keys to scripts or diagnostics.

**AUTH-003 — WebAuthn.** Implement the stateful engine UX: account selection,
PIN entry, touch/security-key instructions, failure, retry, cancel, completion.
Bind it to the original request and relying-party ID. Clear PIN buffers where
practical and never record them in logs, macro history, or screenshots. Qualify
USB FIDO2 with the actual baseline. Bluetooth, phone/hybrid, platform, and
synced passkeys have separate capability entries; unsupported transports must
produce intelligible outcomes.
[WebAuth UX API](https://doc.qt.io/qt-6/qwebenginewebauthuxrequest.html)

**AUTH-004 — TLS.** Certificate failures are blocking by default. Allow a
session-only exception only when the engine marks it overridable and the user
confirms the exact host/error in native chrome. Never bypass HSTS or silently
ignore certificate errors for Google. Preserve secure-context and mixed-content
restrictions. Test local self-signed development sites without changing the
system trust store.

**SEC-005 — Secret storage boundary.** V1 has no built-in password vault. An
explicit external password-manager integration may be added using its supported
CLI/native APIs, with separate review and exact-origin checks. Ordinary
userscripts must not be advertised as safely sandboxed credential handlers.

**SEC-006 — Website data clearing.** `site-data-clear` must report which
categories the pinned engine can clear per origin. Cookies, caches, service
workers, and local storage are not interchangeable. Do not delete private
engine files to simulate a missing per-origin API, and never silently clear an
entire profile when one origin was requested. Offer a separately confirmed
whole-profile reset if needed. Tests verify the stated scope with two sites.

**SEC-007 — Diagnostic experiment boundary.** Site Doctor experiments are
explicit, tab/document-scoped, time-bounded, visible while active, and
individually undoable. They may temporarily retry with the network blocker
off, browser userscripts off, supported site settings reset to their compiled
defaults, or a fresh same-profile page view when that scope is accurately
described. They must not weaken TLS, bypass HSTS, grant a permission, expose a
cross-profile cookie, clear site/profile data, disable the renderer sandbox,
change the global proxy, or persist a compatibility quirk. Navigation, tab
close, profile shutdown, or undo invalidates the experiment. Converting a
successful experiment into a durable rule is a separate previewed action.

## 17. Downloads, files, PDF, and printing

**FILE-001 — File selection.** Connect engine file requests to the desktop
file chooser, preserving multiple-file mode, MIME/extension filters, directory
selection where supported, and cancel. Prefer the toolkit's correct portal
integration; if absent, use a direct FileChooser portal adapter and return only
paths/URIs the engine can accept. Do not create duplicate dialogs. Reject
unsupported remote portal URIs with a precise message or an explicitly
implemented staging flow. Input filenames and paths may contain Unicode,
spaces, quotes, and leading hyphens.
[FileChooser portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)

**FILE-002 — Download lifecycle.** Model offered, selecting-destination,
in-progress, paused (only if supported), completed, interrupted, cancelled.
Retain the engine request safely until destination selection completes. Show
received bytes, total if known, speed as an estimate, and failure reason.
Private downloads have no durable browser index but save an explicit user
chosen file normally. On shutdown, active downloads get a clear continue/cancel
decision before the process exits.

**FILE-003 — Filename safety.** Treat server-supplied names as basenames only:
remove path separators, NUL, dangerous control characters, and platform special
names; bound length while preserving extension. Never append a filename to a
directory without validation. Use a selected destination and collision policy.
Default collisions ask; automatic rename uses numbered unique names. Do not
silently overwrite or follow an attacker-controlled symlink at commit time.
Where the engine writes paths directly, qualify its overwrite behavior and use
a private staging directory plus a safe final move if necessary; include a
same-filesystem/cross-filesystem test and disk-space handling.

**FILE-004 — Open and reveal.** Completed downloads open only after explicit
user action through the desktop handler. Executable content is never executed
automatically. Reveal selects the downloaded file if the desktop supports it,
otherwise opens the containing directory. A failed/cancelled download cannot
be reported as complete because a partial file exists.

**FILE-005 — Save/PDF/print.** Use the engine's built-in PDF viewer and print-to-
PDF support as the initial implementation. Browser page hinting need not work
inside engine-owned PDF UI, but ordinary navigation, copy, zoom, download, and
printing must be documented and tested. Physical printing uses a supported
native/portal print workflow; if Quick lacks the necessary direct print path,
render to a private temporary PDF and use a qualified desktop print path.
Clean up temporary files when safe. A missing printer is distinct from failure
to generate a PDF.
[Qt WebEngine features](https://doc.qt.io/qt-6/qtwebengine-features.html)

**FILE-006 — Spellcheck and menus.** Resolve system language to installed
dictionaries, report missing dictionaries without silently downloading them,
and provide spelling replacements through supported engine operations.
Native context menus include link open/copy, selection copy, download image/link,
inspect, spelling, and relevant edit actions. They share command handlers with
keyboard UI rather than implement separate behavior.

## 18. Media, calls, and Google compatibility

### 18.1 Meaning of supported

**COMPAT-001 — Qualification unit.** A passing result is tied to application
commit/package, Qt build and Chromium security-patch level, distribution
snapshot, compositor version, GPU/driver, portal backends, PipeWire/WirePlumber,
and test date. "Based on Chromium" is not compatibility evidence. Run a local
fixture first, then the live service. Compare failures with the same Qt engine's
reference example to distinguish adapter defects from upstream issues.

**COMPAT-002 — Core Google gate.** Gmail, Calendar, Drive, Docs, Sheets, Meet,
and YouTube must pass the mandatory scenarios below on the primary baseline
before V1. Google account sign-in and persistent authenticated restart are
required where the account/site normally issues persistent login state. A
Google challenge caused by test-account policy is documented separately from
an application failure; it is not automatically a pass.

| Test ID | Service | Procedure and pass condition |
| --- | --- | --- |
| G-01 | Account sign-in | Fresh work profile, sign in with authorized test account and required MFA; redirects/popups finish without browser error |
| G-02 | Session persistence | Close normally, restart same profile, account persists according to server policy; private profile does not survive destruction |
| G-03 | Account isolation | Sign different accounts into two profiles; confirm no cookie/account chooser crossover |
| G-04 | Gmail | Search, open conversation, compose draft, edit with shortcuts/IME, attach chosen file, download attachment; send only to explicit test recipient |
| G-05 | Calendar | Create/update/delete test event, switch views, timezone display, permission prompting if requested |
| G-06 | Drive | Upload files including Unicode name, create test folder, download, preview PDF, handle picker cancellation |
| G-07 | Docs | Create/edit disposable doc, select/copy/paste plain and rich content, undo/redo, IME, comments; no mode stealing |
| G-08 | Sheets | Edit cells, formulas, multiple selections, copy/paste range, large-sheet scrolling, resize window/move outputs |
| G-09 | Meet devices | Join test meeting, grant mic/camera, verify send/receive with second participant, switch devices, mute/unmute, leave |
| G-10 | Meet screen share | Share selected display via portal; second participant confirms live content; stop from browser and portal, verify termination |
| G-11 | Meet window share | Share a chosen window when qualified portal exposes it; verify correct source and cancellation; mandatory on primary Hyprland baseline |
| G-12 | Meet resilience | 60-minute call with media, 15-minute share, resize/move outputs, tab switch; UI responds and no unexpected capture loss |
| G-13 | YouTube | Play qualified VP9/AV1/H.264 samples as available, seek, fullscreen/escape, audio mute, media keys; verify baseline H.264 where package supports it |
| G-14 | Notifications | Grant/deny/revoke on supported service, verify desktop click routes to original profile/window; no duplicate notifications |
| G-15 | Authentication UX | FIDO2 key account test where enabled; test cancel/PIN/touch/retry through engine UX |
| G-16 | Editor reload/recovery | Deliberately close/recover disposable Docs/Sheets tabs; browser never claims to have persisted unsaved page form state |
| G-17 | Standard popups | Local OAuth-style opener/redirect/close fixture plus authorized real flow; preserve opener and profile |

G-11 is a release gate for the stated primary experience. If the available
Qt/portal combination cannot provide it, obtain a product scope decision or
fix the dependency; do not label whole-screen capture as passing window share.
G-15 needs physical/authorized account evidence; a mock only tests the UI state
machine. Test report fields record `pass`, `fail`, `blocked`, or `not-run`.

**COMPAT-003 — Credentials and external effects.** Automated PR CI never holds
real Google credentials. Use a separately authorized test account/environment
for manual or restricted qualification, with disposable documents and a
dedicated meeting. No bulk account creation, personal mailbox access, or
unapproved messages are part of the test plan. Record sanitized outcomes, not
cookies, tokens, recordings of PIN entry, or private account content.

### 18.2 Capture and media behavior

**MEDIA-001 — Screen sharing ownership.** WebRTC capture stays inside the
engine's supported capture path. Qt/Chromium's portal integration obtains the
PipeWire stream; Rust displays browser permission/capture state and handles
engine request UI where necessary. Do not separately acquire a PipeWire stream
and assume it can be injected into WebEngine without a supported API. Trace
the actual request chain in M0, including duplicate-prompt prevention and
portal cancellation. Capability differences are recorded precisely.
[ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)

**MEDIA-002 — Consent and visibility.** Capture begins only after the relevant
engine consent and portal selection. Browser chrome shows an always-accessible
capture indicator, requesting origin, and stop action. Screen source consent
does not persist across sessions. A portal restore token, if used by upstream,
does not authorize silent new capture. Closing the tab must end capture.

**MEDIA-003 — Device lifecycle.** Test unplug/replug, device switch, permission
deny/revoke, portal service restart, and call termination. One failed request
must not leave the whole profile unable to ask again. Keep audio/capture tabs
active. System audio sharing is a separately reported capability; do not
promise it merely because screen sharing and microphone input work.

**MEDIA-004 — Codec and DRM packaging.** The package capability manifest lists
enabled codecs and the outcome of actual decode tests. Standard conferencing
and YouTube qualification use lawful build/package options. Widevine and
premium streaming support are optional and separately qualified; Qt's DRM
hooks do not supply redistribution rights or a CDM binary. Never download a
CDM from an unverified unofficial source or promise Netflix based only on a
loaded library.

**MEDIA-005 — Media session integration.** Prefer the engine's existing media
key handling; implement an MPRIS bridge only for actions/state exposed through
supported APIs or a carefully scoped adapter. Do not run both paths and double
toggle playback. Advertise only supported actions. Private tabs omit title/URL
metadata by default. Page picture-in-picture and document picture-in-picture
must each be capability-checked; a floating web window is not falsely labeled
equivalent.

**MEDIA-006 — Notifications versus push.** On-page web notifications are bridged
to desktop notifications after origin consent. Push is separately disabled by
default and can be enabled per normal profile with a clear explanation of the
service connection and browser lifetime. Do not promise notifications after
the browser process exits without an implemented background service. Qt's push
implementation uses Firebase Cloud Messaging.
[Quick profile push support](https://doc.qt.io/qt-6/qquickwebengineprofile.html)

### 18.3 Compatibility policy

**COMPAT-004 — User agent.** Begin with the qualified engine's normal user agent.
If live evidence shows a compatibility problem due to an application-specific
suffix, a narrowly reviewed upstream-aligned adjustment may be applied. It
must describe the actual engine family and must not advertise unsupported
capabilities. Never globally impersonate a newer Chrome as a substitute for
updating Qt. Version reporting always exposes the true engine build.

**COMPAT-005 — Fast upstream diagnosis.** Reproduce regressions on a minimal
Qt reference browser, preserve sanitized reproduction steps, file/link an
upstream issue when authorized, and keep any local workaround bounded by
engine versions and exact affected site pattern. Test removal when upstream
fixes arrive. The current qutebrowser changelog is a useful regression watch
source, but its workarounds are not copied blindly.
[qutebrowser changelog](https://github.com/qutebrowser/qutebrowser/blob/main/doc/changelog.asciidoc)

## 19. Blocking, network policy, and compatibility quirks

**NET-001 — Network blocking.** Use a maintained Rust ABP-compatible request
filter with EasyList/EasyPrivacy as the initial named list sources. Verify the
actual crate's parser, domain handling, supported rule subset, and license.
The pinned package includes a known list snapshot or a clearly labeled empty
state requiring explicit fetch. Recommended V1 packaging includes a snapshot
so blocking works offline on first use. Display source date and enabled lists.

**NET-002 — Updates.** Named list updates use HTTPS and conditional requests
where available, with download-size limits (32 MiB per source, 128 MiB total),
timeouts, parsing limits, and atomic replacement after successful compilation.
Keep the previous valid snapshot. A malformed/empty/error download must not
erase working rules. Updates occur on explicit command and, by default, at
most once per 24 hours while an ordinary profile is open. Disclose these
maintenance requests; private-only operation does not trigger an update.

**NET-003 — Interception semantics.** Correctly distinguish top-level document,
subframe, resource URL, initiator/first party, request type, and redirects using
engine-provided information. Unknown initiator is not guessed from whichever
tab is focused. If the engine lacks context required for a security deny rule,
deny that controlled operation; ordinary adblocking may allow an unknown
request to preserve browsing, with an explicit diagnostic count. Those two
policies are not interchangeable.

**NET-004 — Exceptions.** Site blocking bypass disables only the blocker for
the exact configured site context. It cannot bypass permissions, TLS, sandbox,
popup policy, or external-scheme restrictions. Provide per-tab blocked request
counts and an on-demand request explanation with safe redaction. Never create
a broad Google allowlist to mask unresolved filtering defects.

**NET-005 — Cosmetic rules.** Cosmetic filtering is POST by default. If added,
its selectors, scriptlets, page-world requirements, performance cost, and
compatibility need separate implementation and tests. A working network
blocker must not be described as uBlock Origin parity.

**NET-006 — Compatibility registry.** A workaround entry contains ID, affected
site pattern, engine version range, symptom, supported change, upstream issue,
regression fixture, date added, review/removal condition, and enabled state.
Rules are shipped as reviewed data or code with releases, not downloaded as
unsigned executable patches. `diagnostics` lists active workaround IDs without
disclosing the user's browsing history.

**NET-007 — Browser maintenance traffic.** No telemetry, automatic crash upload,
remote search suggestions, or hidden browsing-history sync. Blocklist updates,
user-configured pages/scripts, explicitly enabled push, and sites' own traffic
are documented separately. No unsupported promise of zero network traffic when
the user starts a configured online page.

## 20. CLI, IPC, userscripts, and external editors

### 20.1 Command-line contract

**CLI-001 — Entry points.** The executable supports these forms, with help
generated from stable command definitions. `open`'s arguments are data, even
when they begin with a colon.

```sh
rustbrowser
rustbrowser open --target tab-bg -- https://example.com
rustbrowser open --target window --profile work -- https://mail.google.com
rustbrowser open --context work -- https://calendar.google.com
rustbrowser open --target private-window -- https://example.com
rustbrowser command --window last-focused -- 'hint links'
rustbrowser command --window last-focused -- 'action url copy --clean'
rustbrowser query tabs --format json
rustbrowser query active-tab --window last-focused --format json
rustbrowser config check
rustbrowser diagnostics --format json
rustbrowser --version
rustbrowser --temp-basedir open -- https://example.com
rustbrowser --basedir /absolute/test-profile open -- https://example.com
```

Supported global options include `--basedir PATH`, `--temp-basedir`,
`--config PATH`, `--profile NAME`, `--context NAME`, `--instance ID`, `--set KEY=VALUE`,
`--log-level LEVEL`, and `--version`. `--basedir` and `--temp-basedir` are
mutually exclusive. `query`, `command`, and `diagnostics` do not silently launch
a GUI if no instance exists, except that standalone diagnostics can report
build/environment facts with unavailable runtime probes clearly marked.
`open` creates an instance when necessary. `config check`, help, version, and
offline queries of configuration must not need a graphical display.

**CLI-002 — Instance selection.** One GUI instance owns a canonical base
directory and associated profiles. Forward normal launches to it through IPC.
The instance key is a stable digest of canonical base directory plus relevant
user/session identity; it is not a raw pathname in a socket name. Separate
Wayland sessions cannot concurrently own the same durable profile; report the
owning session instead of stealing its lock. If multiple explicit instances
match a non-default selection, require `--instance` rather than choose randomly.

**CLI-003 — Exit codes.** `0` success, `1` operation failure, `2` invalid
arguments/config, `3` no matching instance/target, `4` permission or security
policy refusal, `5` unsupported capability, `6` busy/timeout, `7` protocol
version mismatch. Structured output includes a stable symbolic error code and
human-readable explanation. Accepted asynchronous navigation returns success
for accepted dispatch, not for eventual page load; `--wait` with a bounded
timeout is an optional explicit navigation completion mode.

### 20.2 IPC transport and envelopes

**IPC-001 — Transport.** Use an AF_UNIX stream socket inside the validated
0700 instance directory, with socket mode 0600. Check peer UID through Linux
peer credentials. Acquire an OS-held lock for instance ownership and validate
stale socket replacement safely. There is no TCP listener in production.
Browser pages cannot reach the socket through a URL handler or web bridge.

**IPC-002 — Framing.** Each message is four-byte unsigned big-endian length
followed by UTF-8 JSON, maximum 1 MiB. Reject zero/oversized frames before
allocation; maximum JSON depth 32. A connection must complete handshake within
five seconds. Maximum 32 connections and 64 outstanding requests per connection
initially. Close malformed connections cleanly; never panic on parse failure.

Handshake request and response:

```json
{
  "id": "hello-1",
  "method": "hello",
  "params": {"protocol_major": 1, "protocol_minor": 0, "client": "rustbrowser-cli"}
}
```

```json
{
  "id": "hello-1",
  "result": {
    "protocol_major": 1,
    "protocol_minor": 0,
    "instance_id": "c6346caf-e69d-4919-99ca-510c89bb7d32",
    "capabilities": ["command.execute", "tabs.query", "events.subscribe"]
  }
}
```

Command request and accepted response:

```json
{
  "id": "req-42",
  "method": "command.execute",
  "params": {
    "command": "open",
    "arguments": {"target": "tab-bg", "input": "https://example.com/"},
    "context": {"window": "last-focused", "profile": "work"}
  }
}
```

```json
{
  "id": "req-42",
  "result": {
    "status": "accepted",
    "operation_id": "op-42",
    "tab_id": "tab-19",
    "target_window": "win-2"
  }
}
```

Error response:

```json
{
  "id": "req-43",
  "error": {
    "code": "E_UNSUPPORTED",
    "message": "This engine build has not qualified system-audio capture.",
    "details": {"capability": "capture.system_audio"}
  }
}
```

**IPC-003 — Method surface.** V1 supports `hello`, `command.execute`,
`action.execute`, `actions.query`, `windows.query`, `tabs.query`,
`profiles.query`, `contexts.query`, `switcher.query`, `site.status`,
`bindings.query`, `downloads.query`, `config.get`, `diagnostics.get`,
`operations.query`, `operations.cancel`, and `events.subscribe`. Commands and
actions use typed `arguments` objects derived from their registries. Clients
cannot supply trusted source flags, consent assertions, or native event handles.
JSON strings are not reparsed as shell or command chains. An optional
`command.parse` method may validate command text but never execute it as a side
effect. Site Doctor mutation remains a command/action with normal effect-class
and confirmation policy; `site.status` is read-only.

**IPC-004 — Query schema.** Tab results include tab/window IDs, profile ID/name,
optional context ID/name, compositor workspace/output when known, safe URL,
safe title, selected/pinned/muted flags, loading/renderer state, and document
revision. Results omit private tabs unless configuration permits
private queries and the request explicitly sets `include_private: true`.
Private permission answers and secrets are never exposed. Default URL output
removes userinfo and recognized auth secrets; raw-query/fragment output, if
added, requires an explicit sensitive-output flag and is not logged.

**IPC-005 — Event ordering.** Events have instance ID, monotonically increasing
sequence, event type, and sanitized payload. Types include window/tab changes,
navigation state, download progress, config revision, mode change, and operation
completion. Snapshot queries return their sequence watermark so subscribers
can detect a gap and refresh. Bound subscriber queues to 256 messages or 4 MiB;
coalesce progress or send a gap marker and disconnect a persistently slow
consumer. Security and command completion events are not silently dropped.

**IPC-006 — Retry semantics.** Within one live connection, cache request IDs
and results for 60 seconds (maximum 1,024) to avoid duplicate side effects on
same-ID retransmission. Same ID with different payload is an error. Across
disconnect/restart, no exactly-once guarantee is made; clients query operation
state or ask the user before retrying a destructive command. Do not promise
durable operation replay in V1.

### 20.3 External commands and userscripts

**SCRIPT-001 — Process execution.** `spawn` uses direct executable/argv APIs,
never an implicit shell. Token placeholders include `{url}`, `{title}`,
`{selection}`, and `{hint_url}`. Resolve each as a string value after parsing;
do not splice it into command syntax. Empty/missing values are explicit errors
where required. Use a controlled working directory and document inherited
environment variables. URLs and titles never decide the executable path.

For `:spawn -- mpv {url}`, the URL is one argv value. Advanced users may
explicitly run a shell themselves, but generic placeholders must not be
promoted as safe shell quoting. Keep example integrations shell-free.

**SCRIPT-002 — Userscript registration.** Installed userscripts have a manifest
with name, executable path, argv, requested context fields, allowed result
actions, optional typed action registrations, timeout, and whether private
contexts are allowed. Referenced scripts are local trusted programs;
installing/enabling one is an explicit user act.
The manifest limits the browser's protocol response, not the executable's OS
permissions. No native in-process plugin loader exists in V1.

A manifest is a `<name>.toml` file under the configured userscripts directory.
Relative executable paths resolve against that manifest's directory, never
the current page or working directory. Example target contract:

```toml
schema_version = 1
name = "video"
executable = "./video"
argv = []
context_fields = ["url", "hint_url"]
allowed_results = ["message"]
allowed_commands = []
timeout_seconds = 30
allow_private = false

[[actions]]
id = "userscript.video.send-link"
subject = "link"
verb = "send"
label = "Send link to video player"
required_fields = ["url"]
```

Here `video` is a user-installed wrapper that reads the userscript protocol;
mpv itself does not speak that protocol. Direct `spawn -- mpv {url}` needs no
wrapper. Reject unknown context fields/results/commands, missing executables
at invocation, and manifests larger than 64 KiB. Changing executable path or
requested browser capabilities is a new trusted installation/configuration
action, not something a page or script result can perform. Action IDs are
globally unique, stable, and namespaced. The manifest can bind a declared
subject and typed context fields to this userscript invocation; it cannot
define command grammar, inject UI code, gain unlisted fields, or override a
built-in action. The same action metadata supplies palette, hint, context-menu,
IPC, and launcher presentation.

**SCRIPT-003 — Userscript protocol.** Spawn with a one-shot versioned JSON
context on stdin, then close stdin. Provide simple `RB_URL`, `RB_TITLE`,
`RB_MODE`, and `RB_PROFILE` environment variables only for fields the manifest
requests; prefer JSON for selection or multiline content. Supply the original
tab/document token in the structured context, not an unrestricted control
socket capability. Default timeout is 30 seconds, configurable up to 300.

```json
{
  "protocol_version": 1,
  "invocation_id": "script-7",
  "context": {
    "tab_id": "tab-19",
    "document_revision": 8,
    "profile": "work",
    "private": false,
    "url": "https://example.com/",
    "hint_url": "https://example.com/video"
  }
}
```

Read line-delimited JSON responses from stdout, with 64 KiB maximum per line
and 1 MiB total. Stderr is shown on explicit request with bounded redaction;
it is not a command stream. Allow result types `message`, `open`, `yank`, and
`command` only if explicitly listed in the manifest. A `command` result is a
typed command/arguments object restricted by its manifest allowlist. Do not
allow script-returned permission consent, unconfirmed destructive operations,
or arbitrary executable changes through an innocuous message result.

**SCRIPT-004 — Cancellation.** Cancelling a userscript sends terminate to its
tracked process group, waits a bounded interval, then offers/uses the documented
kill policy. Drain/close streams safely. Navigation invalidates document-bound
results. An external media player launched as a detached explicit `spawn` is
not automatically killed when its originating tab closes. Those are distinct
process lifetime modes.

**SCRIPT-005 — Context privacy.** Do not export cookies, auth headers, passwords,
hidden fields, or entire DOM by default. Access to page HTML or private-profile
data requires a declared capability and explicit enablement. User-requested text
selection is capped at 1 MiB. Installed scripts can access the user's OS
resources outside this protocol; document that accurately.

### 20.4 External editor

**EDITOR-001 — Supported editing.** Extract a focused textarea or supported
plain-text contenteditable field into a 0600 file within a private runtime
directory. Execute the configured editor argv with the file as its own
argument; respect a terminal launcher only as explicit argv configuration.
Never extract password fields. Rich/canvas editors that cannot round-trip
plain text reliably return unsupported rather than destroy formatting.

**EDITOR-002 — Write-back.** Capture profile/tab/document/frame/element and a
hash of the original value. When the editor finishes, validate target identity
and check that the field has not independently changed. On conflict, offer
review/copy/cancel rather than overwrite. Apply text through supported page
operations with input/change events as appropriate, and never submit the form.
Private editing requires an explicit warning that the editor may create its
own swap/backup files. Remove browser temporary files after completion/cancel
when safe, and handle leftovers through exact managed-directory cleanup.

`config-edit` is the separate file-backed configuration flow: it refuses when
there is no active regular configuration file or no valid `tools.editor` argv,
passes the trusted path only through the declared complete `{file}` argument,
and owns the child process until exit/cancellation. A successful editor exit
reuses the debounced configuration watcher and atomic last-known-good commit;
the command never applies partial edits directly or falls back to `EDITOR`,
`VISUAL`, a shell, or an implicit editor.

## 21. Wayland and desktop integration

**WL-001 — Native operation.** Supported GUI startup selects Qt's Wayland QPA
backend and verifies it at runtime. If the selected backend is XCB/XWayland,
show an unsupported-configuration error or explicit development override; do
not report a native Wayland test pass. Headless core/CLI tools remain usable
without a display. Do not globally export Qt environment variables into the
user's shell to repair one application.
[Qt Wayland clients](https://doc.qt.io/qt-6/wayland-and-qt.html)

**WL-002 — Scaling.** Qualify 100%, 125%, 150%, and 200% scale, mixed-DPI monitor
movement, resize, output removal, and page zoom combinations. Web rendering,
hints, context menus, popup positions, text cursor, and hit testing must agree.
Leave protocol negotiation to Qt unless a concrete missing capability requires
a narrow adapter. No direct compositor framebuffer access.

**WL-003 — Input and clipboard.** Support regular clipboard, primary selection
when available, drag/drop, IME, touchpad smooth scrolling, keyboard layouts,
and standard pointer behaviors. Clipboard availability is a capability; a
missing primary-selection protocol yields an explicit message and optional
normal-clipboard fallback only when requested. User gesture clipboard flows
must be distinguished from website clipboard permission requests.

**WL-004 — Activation.** Use Qt/xdg-activation support and valid desktop-launch
activation tokens where available. IPC focus requests can be denied by the
compositor; return the actual outcome/unknown status without repeated forced
focus attempts. Do not forge activation tokens, manipulate another app's
window, or depend on X11 window IDs. Background-tab opens must stay background.

**WL-005 — Portal capabilities.** Probe portal interfaces and relevant versions,
not merely whether `xdg-desktop-portal` has a running process. FileChooser,
ScreenCast, OpenURI, and notification paths are separate capabilities. Use
asynchronous requests with parent-window identification where available,
timeouts, and cancellation. Show the selected backend information when it can
be determined safely. A failed file picker may offer a native fallback in auto
mode; screen sharing never falls back to unauthorized capture.

**WL-006 — Missing services.** If portals/PipeWire are unavailable, normal
browsing still works and the affected operation reports a concrete missing
component. Provide diagnostic guidance, not automatic service restarts or
machine configuration writes. On the primary supported install, package/docs
must arrange the required services and qualification verifies their behavior.

**WL-007 — Desktop conventions.** Install a desktop file with a stable app ID,
icons, standard URL and HTML associations, and normal/private/window actions.
Use XDG user directories for downloads, desktop handlers for external URIs,
and freedesktop notifications or the selected portal path. Default-browser
registration is an explicit user operation. Do not change it during install
or first launch without consent.

## 22. Hyprland and Omarchy integration

### 22.1 Hyprland adapter

**HYPR-001 — Optional boundary.** Put Hyprland-specific behavior behind a runtime
capability adapter. Auto mode checks the compositor and socket availability;
off mode performs no Hyprland requests. General browsing never depends on a
Hyprland response. Use the instance signature and runtime directory, validate
ownership, and handle compositor restart/reconnect with bounded backoff.
Hyprland publishes separate request and event sockets.
[Hyprland IPC](https://wiki.hypr.land/IPC/)

**HYPR-002 — Window identification.** Use the provisional development desktop
ID `io.github.rustbrowser.RustBrowser` consistently in executable metadata and
packaging. Confirm the final owned namespace before public release. Map browser
windows to compositor windows using stable supported identifiers where
available, not page title matching or a process PID alone. Multiple browser
windows share a process, so PID-only matching is insufficient.

**HYPR-003 — Workspace routing.** Workspace placement is opt-in. A route names
a browser context or profile and a workspace target, not arbitrary raw
Hyprland command text. Context wins when both are applicable, but its profile
must equal the window profile. Validate targets and send structured/escaped
supported requests. Wait
for the intended window to map and match it deterministically; if mapping is
ambiguous, report failure rather than move the wrong window. Prefer static
documented rules where they meet the requirement. Opening one browser window
must not apply a workspace rule to every browser window.

**HYPR-004 — No competing window manager.** Compositor owns geometry, focus,
borders, workspaces, and tiling. Browser records optional workspace intent for
restoration but does not force unavailable outputs or absolute coordinates.
Provide version-qualified example rules and keybindings. Do not overwrite
hyprland.conf or install global keybindings automatically. V1 does not
implement a browser-specific split-view tiler: it makes independent windows
as searchable, addressable, movable, and restorable as tabs, then lets
Hyprland compose them. A later in-page or accessibility use case may revisit
split view only through a separate product decision.

### 22.2 Omarchy theme and launcher integration

**OMA-001 — Theme source discovery.** In auto mode prefer a valid, configured
Omarchy provider when detected, else system/builtin appearance. Explicit
`theme.path` wins over discovered paths. Support versioned provider adapters:
current inspected Omarchy `quattro` source uses
`~/.local/state/omarchy/current/theme/colors.toml`; older setups have used
`~/.config/omarchy/current/theme/colors.toml`. Treat those as detected layouts,
not a promise that every Omarchy version honors XDG variables identically.
An override is always available. Record tested Omarchy version/commit.
[Omarchy theme source](https://github.com/omacom/omarchy/blob/quattro/bin/omarchy-theme-set)

**OMA-002 — Data-only import.** Read and validate palette TOML, never source a
theme shell file or execute code from a downloaded theme. Map background,
foreground, accent, selection, and semantic red/yellow/green tokens into the
browser theme. Resolve missing colors through documented fallbacks. If using
an official color-resolution helper, it must be a detected trusted installed
program, called with direct argv, bounded time/output, and an adapter-version
test. The default implementation can parse a supported palette schema in Rust.

**OMA-003 — Theme changes.** Watch the current theme's parent directory and
re-establish watches after replacement/symlink changes. Reload atomically and
keep the prior palette while a transition is incomplete. Optional integration
hooks invoke `rustbrowser command -- theme-reload` using the installed Omarchy
hook contract; do not assume every version executes hooks by their shebang.
No per-frame polling. Theme reload must not reload pages, reset modes, restart
Meet, or rewrite the user's browser config.

**OMA-004 — Launcher and shell.** Ship optional desktop actions and examples
for the launchers/shell supported by the tested Omarchy version. Invoke the
public CLI/IPC rather than scrape titles or inspect browser databases. Tab
search returns normal-profile results with stable IDs; selecting a result
activates that exact tab. Never require Walker, Waybar, or a particular shell
implementation as a core dependency because Omarchy components can change.

**OMA-005 — Integration package.** The optional `rustbrowser-omarchy` package
contains declarative themes/templates, versioned example Hyprland rules,
launcher actions, and a doctor procedure. Install vendor resources into
package-owned locations. User activation copies/includes narrowly scoped
resources explicitly; upgrades do not overwrite user modifications. The
integration identifies itself as compatible/community-maintained unless the
Omarchy project actually endorses it.

**OMA-006 — This workspace's customization workflow.** During development on
the current machine, accepted system/desktop changes must follow its supplied
AGENTS.md: implement and validate, obtain post-change acceptance, then manage
relevant non-secret user files in `/home/tom/dotfiles/home`, run the Stow
installer, snapshot curated root config when relevant, review the full diff,
commit only accepted customization changes, push, and verify main matches
origin/main. Application source remains in this repository. Do not treat
approval of this specification as acceptance of future machine behavior.

## 23. Accessibility and international input

**A11Y-001 — Chrome accessibility.** Give command input, completion rows, tabs,
permission prompts, menus, mode labels, download controls, and capture controls
accessible names, roles, states, and focus order. Announce meaningful changes
without reading every progress update. Keyboard operation alone is insufficient
evidence for screen-reader accessibility.

**A11Y-002 — Focus and navigation.** A user can leave every browser-owned
overlay with Escape, reach primary actions without a mouse, and recover focus
after a portal/dialog closes. No invisible overlay traps focus. Native text
fields preserve selection/editing conventions. Caret mode does not replace
the engine's accessibility tree.

**A11Y-003 — Contrast and motion.** Builtin themes target WCAG AA contrast for
normal-size chrome text; test actual rendered colors. Mode/security/error
state is not color-only. User themes can be imported even if imperfect, but
security prompts retain readable foreground/background and diagnostics can
identify poor contrast. Respect reduced motion and system font scaling.

**A11Y-004 — Input matrix.** Qualify US and one non-US layout with AltGr/dead
keys; at least one CJK IME; Unicode search/path/title handling; bidirectional
text in chrome; clipboard round trips; page editors after hint activation; and
Ctrl/Shift combinations with layout switching. Use public Qt input paths.
Initial documentation may be English, but strings must be separated from
logic so localization can be added without rewriting command behavior.

## 24. Diagnostics, failures, and recovery

**DIAG-001 — Report contents.** `diagnostics` and a native diagnostics view show:
application version/commit, Rust/bridge build, Qt compile/runtime versions,
Chromium base and security-patch version, native Wayland verification,
compositor/version, selected Qt graphics backend, GPU/driver information where
available, sandbox probe status, codec/DRM status, portal interfaces, PipeWire
availability, dictionary inventory, storage health, active workaround IDs,
and recent error categories. Each field records whether it was observed,
configured, or not probed. Do not infer an exact patch level from a user agent.

**DIAG-002 — Report privacy.** Default output excludes browsing URLs, account
names, cookies, credential paths, complete home-directory paths, page console
logs, and private-session data. Explicit troubleshooting sections show a
preview and retain redaction. JSON output has a versioned schema. There is no
automatic network submission or silent core-dump upload.

**DIAG-003 — Error vocabulary.** Stable errors include `E_INVALID_ARGUMENT`,
`E_CONFIG`, `E_NOT_FOUND`, `E_NO_INSTANCE`, `E_STALE_TARGET`, `E_DENIED`,
`E_CONFIRMATION_REQUIRED`, `E_UNSUPPORTED`, `E_BUSY`, `E_TIMEOUT`, `E_PROTOCOL`,
`E_IO`, `E_STORAGE`, `E_ENGINE`, and `E_CANCELLED`. A user message explains
what failed, what was preserved, and a concrete next action. Logs retain a
correlation ID instead of confidential raw input.

**DIAG-004 — Site ledger.** `site-status` and the native Site Ledger report the
captured tab/document, normalized origin, profile and context, effective
setting values with provenance, permission state and pending requests, active
capture, blocker/list decision counts, installed page scripts, compatibility
quirks, service-worker/site-data management capabilities, and relevant engine,
codec, portal, or GPU limitations. Unknown and unavailable are distinct from
false. The default view summarizes the current origin; expanding request-level
detail is bounded and redacted, and it never exposes cookies, tokens, form
content, or another profile's state.

**FAIL-001 — Renderer crash.** A terminated renderer produces an affected-tab
error surface with reload/close/copy-safe-URL and diagnostic actions. Other
healthy tabs remain usable. Track repeated crashes by safe internal identity;
after two failures within 60 seconds, do not auto-reload into a loop. Report if
multiple tabs shared the failed renderer. Restoring the browser process is a
separate session-recovery path.

**FAIL-002 — GPU fallback.** Start with the qualified Qt/Chromium graphics path.
If startup/rendering fails, offer an explicit software-rendering restart with
a fresh diagnostic marker, retaining profile data and sandbox settings. Do
not silently persist experimental GPU flags. Qualify software rendering for
basic browsing, and clearly report degraded media/performance capabilities.

**FAIL-003 — Feature failures.** Missing script/editor returns a command error;
missing portal reports the affected interface; filter compile failure retains
last rules; invalid config retains last good revision; DB failure preserves
data; clipboard failure does not open empty/garbage URLs. A noncritical
integration failure must not become an application crash.

**FAIL-004 — Recovery mode.** Provide a diagnostic `--safe-mode` that uses a
temporary profile, builtin config/theme, no user scripts, and normal security
settings. It must not reset a normal profile or imply security protections are
disabled. Offer a separately named way to launch an existing profile with
user scripts disabled only after its precise behavior is documented.

**FAIL-005 — Process crash diagnostics.** Preserve sanitized structured logs
and a crash marker. Document Linux core dumps as potentially containing
sensitive page/credential memory and never collect/export them automatically.
Supply debug symbols for release builds where packaging permits. Correlate
Qt helper versus browser process crashes without claiming a Rust rewrite
prevents crashes in C++/GPU drivers.

## 25. Performance and resource budgets

**PERF-001 — Reference environment.** M0 records a fixed x86_64 reference
machine with at least four CPU cores, 16 GiB RAM, SSD, a qualified Mesa-based
GPU, native Hyprland, and exact package versions. Report CPU governor, display
resolution/scale, build mode, cold versus warm caches, and process tree. Add
Intel and NVIDIA qualification separately; do not compare unrelated hardware
as a performance improvement. Use release builds for user-facing budgets.

**PERF-002 — Measurement rules.** Use monotonic clocks, at least 30 samples for
short operations, median/p95/p99, and raw result artifacts. Attribute Rust/UI
time separately from engine/render/network time. Measure PSS for the whole
process tree to avoid counting shared memory repeatedly. Do not call `load`
event time a measure of page usability without defining the page fixture.

Initial budgets below are engineering targets to validate in M0/M1 and adopt
or revise with an ADR and baseline evidence before release. Safety limits in
other sections remain hard requirements; a performance target cannot justify
disabling sandboxing, skipping fsync for durable confirmation, or discarding
active calls.

| Measurement | Target on reference baseline | Method |
| --- | --- | --- |
| Normal-mode key to visible chrome response | p95 ≤ 20 ms, p99 ≤ 50 ms | Instrument event arrival/model update/presentation |
| Local command dispatch excluding async effect | p95 ≤ 2 ms | Core and GUI instrumentation |
| Completion over 100k history entries | p95 ≤ 50 ms, newest result visible ≤ 100 ms | Seeded profile, warm indexed query |
| Hints on 500 visible candidates | p95 ≤ 200 ms | Fixed DOM fixture, three page zoom levels |
| Warm process to interactive blank window | p95 ≤ 1 second | 30 launches, no network/start pages |
| Cold process to interactive blank window | p95 ≤ 2.5 seconds | Explicit cache definition, at least 10 launches |
| Empty idle browser CPU | < 1% of one core averaged over 60 seconds | After 30-second settling period |
| Empty browser total PSS | ≤ 400 MiB initial target | Include helpers; record Qt reference comparison |
| Request filtering | p95 ≤ 1 ms per request, no GUI wait | Compiled standard lists, representative corpus |
| Theme reload | visible ≤ 250 ms after stable file event | No page reload or document revision change |
| Durable bookmark acknowledgement | p95 ≤ 100 ms on reference SSD | Includes transaction/fsync |
| Session recovery checkpoint | normal loss window ≤ configured 5 seconds | Injected crash at randomized times |
| Close/open stability | no unbounded retained-tab growth | 1,000 local tab cycles, baseline after warmup |

**PERF-003 — Leak criteria.** After warmup, compare process/object counts and
memory plateaus across repeated identical cycles. No stale QObjects, requests,
observers, timers, or tab model entries may accumulate per cycle. Retained
engine caches must have a documented bound. A linear positive per-cycle memory
slope is a failure even if the run has not exhausted RAM. Use heap/Qt object
diagnostics to distinguish an application leak from stable shared caches.

**PERF-004 — Responsiveness under load.** With 100 lazily restored tabs, a
large history DB, active download, and a meeting tab, switching browser modes
and opening command completion must remain responsive. Expensive operations
are cancellable or display bounded progress. Never process an unlimited IPC
or script-output queue in one event-loop turn.

## 26. Test architecture and qualification matrix

### 26.1 Test layers

**TEST-001 — Pure core.** Unit and property tests cover command parsing,
keychain/count handling, mode transitions, URL policy, site-rule precedence,
profile/tab invariants, stale target rejection, permission decisions, session
descriptor eligibility, and serialized protocol compatibility. They need no
Qt installation, display, or network. Tests should assert observable contracts
and invariants rather than mirror implementation line by line.

**TEST-002 — Adapter integration.** Real Qt tests verify signals, callbacks,
QML models, profile ownership, file requests, popup adoption, script worlds,
request interception, and teardown. Test cancellation before/after response,
view destruction during callback, and profile shutdown with queued operations.
Use debug Qt assertions and sanitizers where compatible with the toolchain;
exclude neither the C++ bridge nor Rust FFI merely because Rust tests pass.

**TEST-003 — Native Wayland harness.** Launch a disposable nested compositor
with a private runtime directory and test D-Bus session. Exercise an actual Qt
Wayland window. A headless WebEngine test or Xvfb run is not a substitute for
Wayland coverage. Portal mocks validate request schemas/cancellation but real
portal/PipeWire tests qualify capture. Avoid globally changing the developer's
session environment or restarting their desktop services.

**TEST-004 — Local web fixtures.** A deterministic fixture server supplies HTTP
and HTTPS endpoints on separate origins with test-only certificate handling,
redirects, authentication challenges, WebSockets, service workers, downloads,
file forms, popups, permission pages, editable text, shadow DOM, frames, and
synthetic heavy pages. Test credentials are obviously fake and restricted to
fixtures. Do not weaken production TLS logic to trust fixture certificates.

**TEST-005 — UI automation boundary.** Use public Qt test input and native
accessibility hooks where useful. A test-only adapter may inject typed core
events, but real key/IME/focus behavior needs native input tests too. Any
automation socket or Chromium remote-debugging port is disabled in production
and scoped to a disposable profile/session. CDP, if used, is local only and
cannot by itself verify native chrome or portal behavior.

### 26.2 Required regression scenarios

| Test ID | Scenario | Required assertions |
| --- | --- | --- |
| T-001 | Keychain ambiguity, timeout, count overflow | One deterministic command or cancellation; no page typing |
| T-002 | Insert/normal/pass-through plus IME | No lost composition, stuck modifier, or trapped mode |
| T-003 | New permission prompt during typing | Opening key does not grant; origin correct; focus restored |
| T-004 | Command injection corpus | URLs/titles with separators/quotes remain data |
| T-005 | Config include cycle/partial write/unknown key | Old revision remains active; precise error |
| T-006 | Concurrent profile windows | Cookies, history, grants, completion separated |
| T-007 | Popup opener/POST/auth redirect | Same profile, preserved engine semantics, safe close |
| T-008 | Navigation/close while script callback pending | Stale result ignored, no use-after-free |
| T-009 | Frames/shadow roots/zoom/mixed DPI hints | Candidate labels/hit targets match; unsupported targets skipped |
| T-010 | User activation via hints | Correct popup/file/fullscreen behavior without bypass |
| T-011 | Private run and crash | Scan browser-owned durable files for unique private marker; absent |
| T-012 | Simultaneous instance startup | Exactly one owner; loser forwards or errors; no lock theft |
| T-013 | Crash during DB migration/snapshot write | Old or new valid state; no silent empty reset |
| T-014 | Disk full/read-only data/cache | Actionable error; no misleading durable success |
| T-015 | Closed-tab undo/session restore after POST | No implicit form resubmission |
| T-016 | Dangerous download filename/symlink/collision | No directory escape or unconfirmed overwrite |
| T-017 | Portal cancel/timeout/service restart | Requests resolve once; UI stays usable |
| T-018 | Capture revoke/tab close | Stream actually stops; indicator matches state |
| T-019 | File chooser multiple/Unicode/folder/filter | Correct selected files or clean unsupported/cancel outcome |
| T-020 | Blocking lists invalid/huge/update interrupted | Old filter snapshot retained; memory bounded |
| T-021 | Two first-party tabs with site exceptions | Blocker uses correct request context, not focused tab |
| T-022 | IPC invalid framing/oversize/disconnect/flood | Bounded resources, no panic, no duplicate side effects |
| T-023 | Script output injection/hang/private access | Manifest response limits, timeout, privacy rules apply |
| T-024 | Editor target changed during external edit | Conflict dialog, no overwrite/submission |
| T-025 | Renderer termination and repeated crash | Affected-tab recovery, other tabs usable, no reload loop |
| T-026 | Theme parent replacement/provider missing | Correct rewatch/fallback; no page or mode reset |
| T-027 | Hyprland two windows same PID | Route/focus exact target or report ambiguity |
| T-028 | Fullscreen, output removal, pointer lock | Recover through reserved escape; usable geometry |
| T-029 | Origin normalization corpus | No wildcard suffix, port, IDN, or opaque-origin grant bypass |
| T-030 | Internal scheme/web bridge attacks | No file traversal, privileged API, or native command access |
| T-031 | Package installed without build tree | Resources, locales, helper, QML, Wayland plugin all found |
| T-032 | Upgrade/downgrade and schema boundary | Upgrade preserves data; unsupported downgrade refuses safely |
| T-033 | Screen-reader chrome interaction | Named controls, correct focus and meaningful announcements |
| T-034 | Notifications/private metadata/media keys | Origin/profile target correct; no private leakage/double toggle |
| T-035 | Permission memory/durable policy | Ask-once/session/persistent/revoke work as documented across restart |
| T-036 | Per-site setting while another tab active | No profile-wide setting races; correct pending apply time |
| T-037 | Print/PDF/download failure | Correct status, overwrite protection, temporary cleanup |
| T-038 | Shutdown with downloads/prompts/dirty DB | Correct order, final flush, crash marker on forced exit |
| T-039 | Same-document navigation | Correct history URL and document-bound callback behavior |
| T-040 | 1,000 tab cycles/24-hour soak | No accumulating objects, runaway queues, or spontaneous process exit |
| T-041 | Context enter/save with profile and Hyprland workspace | Exact context/window restored or created; profile invariant holds; failed placement is visible |
| T-042 | Context route across OAuth popup/redirect/form chain | Suggestion may appear; no automatic profile/context crossing or broken opener state |
| T-043 | Universal switcher across same-PID windows and workspaces | Stable ranked IDs; exact target focused; stale target handled; private excluded by default |
| T-044 | Site Ledger with layered settings/permissions/blocking/quirks | Effective values and provenance match engines/stores; unknown capability labeled accurately |
| T-045 | Site Doctor experiment, navigation, undo, and crash cleanup | Change is scoped/visible/reversible; no TLS weakening, grant, data clearing, or durable residue |
| T-046 | Built-in and userscript typed actions on malicious subjects | One validated action; typed URL remains data; manifest cannot override or escalate capabilities |
| T-047 | Clean-link removal and preservation corpus | Expected trackers removed; signed/auth/payment/deep links preserved; preview and original exact |
| T-048 | Key discovery under overrides, prefixes, modes, and site conflict | Overlay/map/explanation reflect active trie without changing focus, timeout, or dispatch |
| T-049 | V1.1 journey redirect/opener/back-branch/private lifecycle | Correct edge types and recovery; no DOM/form data; private graph never reaches disk |
| T-050 | V1.1 ephemeral profile across windows and final close | Cookies/grants isolated; normal profile unchanged; all transient state destroyed at final owner |

**TEST-006 — Fuzz targets.** Fuzz command lexer/parser, binding parser, IPC
framing/JSON envelopes, action subject/parameter schemas, context routes, site
patterns, clean-link rules, imported bookmark/session/journey data, and
script-result validation. Seed malicious Unicode, deeply nested JSON, huge
numbers, embedded NUL, fragmented frames, repeated IDs, and URL parser
disagreement cases. Store minimized regression cases. Property-test random
valid/invalid sequences of tab events and require STATE-010 invariants.

**TEST-007 — Storage interruption.** Use controlled process termination and
fault-injected I/O to interrupt every migration/snapshot step. Verify SQLite
integrity, snapshot selection, permissions, backup retention, and read-only
recovery. Private marker scans include browser data/state/cache/logs and managed
temporary files, with explicit exclusions only for user-requested output.
OS-owned core dumps are a separate privacy limitation, not browser state.

### 26.3 Environments and cadence

| Environment | Frequency | Coverage/claim |
| --- | --- | --- |
| Linux core build without Qt | Every change | Core/config/IPC deterministic tests, format/lint |
| Primary Qt, nested Wayland, local fixtures | Every integration change | Real adapter/UI behavior; security regressions |
| Current qualified Arch + Hyprland + Omarchy, Mesa AMD or Intel | Every release candidate | Full V1 suite, Google, real portals, install/upgrade |
| Second Mesa GPU vendor | Before V1 and major engine transitions | Graphics, WebRTC, scale, fullscreen, media |
| NVIDIA proprietary driver on native Wayland | Best effort during development; required before claiming NVIDIA support | GPU, scaling, suspend/resume, capture; otherwise explicitly unqualified |
| Sway + its suitable portal/file chooser combination | Every release candidate | Browsing, input, file chooser, screen capture; window capture only if exposed |
| Plasma/GNOME Wayland | When available | Smoke testing; no broad support claim from compilation |
| Candidate Qt minor/patch | On dependency upgrade | Engine regression subset plus Google/capture qualification |
| Software rendering | Each release candidate | Startup/basic browsing/accessibility fallback |
| USB FIDO2 and CJK IME | Before V1 and relevant engine/input changes | Manual hardware/input evidence |

The first release may explicitly limit its supported GPU matrix. A missing
NVIDIA run does not mean it fails; it means support is unqualified. The primary
Hyprland baseline and its Google/Meet scenarios cannot be skipped.

**TEST-008 — Flakiness.** A retry does not erase the original failure. Record
timings and minimized reproduction before classifying environment flakiness.
Quarantined tests have issue, owner, expiry, and release impact. Mandatory
release gates cannot be turned into optional tests through quarantine alone.

**TEST-009 — Accessibility evidence.** Run at least one real screen reader
against browser chrome and a normal page, including permission prompts and
command completion. Check focus transitions manually in addition to semantic
tree assertions. Do not disable Chromium accessibility to hide an upstream
crash without a documented release-level scope decision and workaround plan.

### 26.4 Evidence record template

Store sanitized records under docs/testing/evidence or equivalent CI artifacts.
Retain release-critical records with the release metadata.

```json
{
  "schema_version": 1,
  "test_id": "G-10",
  "status": "not-run",
  "application_commit": null,
  "application_package": null,
  "qt_runtime": null,
  "chromium_base": null,
  "chromium_security_patch": null,
  "distribution_snapshot": null,
  "compositor": null,
  "gpu_driver": null,
  "portal_backends": [],
  "pipewire_version": null,
  "tested_at": null,
  "operator": null,
  "procedure_revision": "1",
  "observed_result": "Awaiting implementation and authorized qualification.",
  "artifacts": [],
  "issue": null
}
```

## 27. Milestones and implementation backlog

### 27.1 Work-item contract

**PLAN-001 — Task cards.** Before implementing a work item, identify its
requirement IDs, dependencies, intended files/modules, observable result,
tests, and exact exit criteria. After implementing it, record changes,
validation, limitations, and next dependencies. Keep commits independently
reviewable when version control is available and authorized. Do not create
source work inside desktop/dotfiles repositories.

**PLAN-002 — Dependency order.** M0 establishes the engine and desktop path;
M1 builds the usable control loop; M2 adds durable data and qutebrowser-style
workflows; M3 qualifies the complete modern-web/security surface; M4 packages,
integrates, and closes release evidence. Accessibility, security, and tests
begin with their first affected feature, not at the final milestone.

### 27.2 M0 — Feasibility and architecture qualification

Goal: a small Rust-driven native Wayland browser prototype that proves the
highest-risk boundaries. It may use temporary UI controls and explicit test
profiles; it must not use insecure production workarounds to pass.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| M0-01 | Workspace, toolchain, Qt/CXX-Qt pin, repeatable build | Clean native build and core-only build documented |
| M0-02 | Rust entry, Qt initialization, Quick window, one WebEngineView | Native Wayland verified; local HTTPS page loads with normal security |
| M0-03 | Typed bridge and lifetime spike | Async callback after close safely ignored; repeated create/destroy passes |
| M0-04 | Two tabs/two windows/explicit profiles | Cookie isolation; popup adopted into opener profile |
| M0-05 | Key/focus/IME spike | Native editable input and guaranteed escape proven |
| M0-06 | Portal file picker and WebRTC capture spike | Real upload, camera/mic, screen and window share chain recorded |
| M0-07 | Google feasibility run | Sign-in, Gmail, Docs, YouTube, Meet basic call/share observed on baseline |
| M0-08 | View migration and scaling | Same-profile live detach spike; 125/150% mixed output hit testing |
| M0-09 | Permission authority and WebAuth UX spike | AskEveryTime mapping, retained requests, cancellation, FIDO2 plan/evidence |
| M0-10 | Capability report and ADRs | API/build/runtime gaps with reproduction; baseline selected |

M0 exit artifacts: runnable prototype, build instructions, package/version
manifest, ADR-001 through ADR-004, capability matrix, sanitized smoke-test
evidence, and an explicit go/no-go report. Unresolved Google sign-in, native
capture, or unsupported required Quick APIs are stop-and-decide gates. Do not
spend months on polish while one of these foundations is unproven.

### 27.3 M1 — Usable keyboard browsing loop

Goal: browse ordinary sites with reliable native input and a command-driven UI.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| M1-01 | Core IDs, reducer, effects, invariants | Pure state/property tests pass without Qt |
| M1-02 | Command registry/parser/errors | Grammar, counts, chains, injection and help tests |
| M1-03 | Normal/insert/command/search/hint/pass-through modes | Default keymap and native focus regression tests |
| M1-04 | Native chrome, tabs, status, completion model | Accessible names; stable viewport; no focus stealing |
| M1-05 | URL resolution/navigation/engine history | Redirect, POST, same-document, search and external-URI cases |
| M1-06 | Basic DOM hints and capture-safe activation | Links/inputs/buttons work; stale target tests |
| M1-07 | Typed config/defaults/live reload | Schema/example validation, last-good transaction, explain output |
| M1-08 | Multiwindow/tab actions and renderer recovery | Close/undo placeholder semantics; crash affects intended tabs |
| M1-09 | Minimal CLI/instance ownership | Open forwarding, no-display help/config check, concurrent launch |
| M1-10 | Continuous Wayland integration tests | T-001 through relevant M1 tests enforced in CI |
| M1-11 | Typed action registry and universal-switcher core | ADR-011 closed; shared action metadata, stable target IDs, async privacy-filtered search |
| M1-12 | Binding discovery and clean-link core | Active-trie explanations; conservative versioned transform corpus |

M1 exits with an interactive alpha suitable for disposable profiles, not a
claim of durable daily-driver readiness. Re-run the M0 Google/input smoke tests
after the normal-mode event filter and overlays are introduced.

### 27.4 M2 — Durable workflows and extension surface

Goal: make the application useful for sustained ordinary work without losing
data or requiring ad hoc integration scripts for basic operations.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| M2-01 | SQLite migrations/worker/profile registry | Transactional interruption, lock, corruption, disk-full tests |
| M2-02 | History/bookmarks/quickmarks/completion | Profile separation, retention, 100k-row performance benchmark |
| M2-03 | Sessions/undo/private memory-only state | Atomic snapshots, no POST replay, private marker scan |
| M2-04 | Complete tab/window/profile operations | Pin/mute/order/move rules; same-profile live movement proven |
| M2-05 | Download manager/file picker/save/PDF | Collision, Unicode, cancel, private output, failure tests |
| M2-06 | Full IPC and CLI contract | Version negotiation, framing/flood/retry/privacy tests |
| M2-07 | Userscripts/direct spawn/external editor | Injection resistance, lifecycle, edit conflict, manifest controls |
| M2-08 | Caret/macros/repeat/advanced hints | Safety exclusions, nested frames, zoom/shadow fixture coverage |
| M2-09 | Network blocker and updater | Working initial lists, atomic updates, per-site exceptions |
| M2-10 | Help/settings/diagnostics managers | Generated command/config docs match implementation |
| M2-11 | JavaScript userscript subset | Script-world boundary, documented metadata/API subset |
| M2-12 | Context registry, saved membership, and routes | One-profile invariant, prompt-only routing, session and conflict tests |
| M2-13 | Universal switcher completion | All V1 target categories/actions, exact multiwindow focus, private filtering |
| M2-14 | Site Ledger and Site Doctor | Accurate provenance/capability view; reversible bounded experiments |
| M2-15 | Typed userscript actions and clean-link UX | Manifest restrictions, palette/hint parity, preview and preservation tests |

M2 exits with a feature alpha and complete persistence/extension contracts.
User data recovery and private browsing tests are mandatory before using a
non-disposable development profile.

### 27.5 M3 — Modern-web and security qualification

Goal: meet the actual work-service promise on the primary platform.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| M3-01 | Full permission service and trusted prompts | Session/durable grants, revoke, navigation cancellation tests |
| M3-02 | Authentication/WebAuthn/client certificate UX | Origin/RP checks; FIDO2 hardware evidence; no secret logging |
| M3-03 | WebRTC/capture controls and lifecycle | Screen/window share, stop/revoke/device switch, no duplicate portal sessions |
| M3-04 | Media/notification integration and codec report | Real media key handling; private metadata policy; truthful capabilities |
| M3-05 | Google qualification matrix | G-01–G-17 completed with required authorized evidence |
| M3-06 | Compatibility registry and engine comparison harness | Every workaround scoped, linked, tested, reviewable |
| M3-07 | Internal scheme/FFI/input security review | Adversarial fixtures and fuzz regressions pass |
| M3-08 | Accessibility and international input qualification | Screen reader, CJK IME, non-US layout, rich editor focus |
| M3-09 | Long sessions and fault injection | 24-hour workload, 60-minute Meet, 1,000 tab cycles |

M3 exits with a beta qualified on the explicitly recorded primary environment.
A live-service test blocked by account access stays blocked; it is not replaced
with a local fixture pass. An unavailable optional DRM system does not block V1
unless the product scope has explicitly added that service.

### 27.6 M4 — Omarchy integration and first stable release

Goal: make installation, upgrades, desktop behavior, support, and maintenance
as reliable as the application itself.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| M4-01 | Arch source package and installed runtime test | All helpers/resources found; clean install and upgrade tests |
| M4-02 | Desktop file, URL handling, app ID, default-browser action | Launch/focus/multiple URLs/Unicode work; consent respected |
| M4-03 | Hyprland/context adapter and examples | Exact-window identification, context workspace intent, routing failure handling, no core dependency |
| M4-04 | Omarchy palette/provider integration | Current and supported older layouts, directory replacement, no code execution |
| M4-05 | Launcher/shell examples and optional package | Public CLI/IPC only; existing user customizations preserved |
| M4-06 | User/developer/support documentation | Fresh user and contributor follow procedures successfully |
| M4-07 | Dependency/update policy and release artifacts | Version manifest, SBOM/license notices, advisory review, checksums |
| M4-08 | V1 traceability review | Every mandatory requirement verified; exceptions explicitly approved |
| M4-09 | Final platform and Google requalification | Same installed release candidate, pinned matrix, no unpublished patch |

V1 is reached only when section 31's release definition is met. These milestone
tables provide sequencing, not permission to omit requirements absent from a
particular work-item row.

### 27.7 V1.1 — Relationship recovery and disposable isolation

V1.1 is a focused feature release, not a reason to defer defects or incomplete
V1 gates. It begins only after V1's supported engine line remains current.

| Item | Deliverable | Exit criteria |
| --- | --- | --- |
| V1.1-01 | Navigation journey graph | ADR-012 closed; transition taxonomy, branch visualization, bounded profile-local retention, T-049 |
| V1.1-02 | Journey recovery actions | Safe descriptor reopening across tabs/windows/contexts; stale node behavior tested |
| V1.1-03 | Ephemeral isolated profiles | In-memory engine/Rust state, multiwindow lifetime, clear UI identity, T-050 |
| V1.1-04 | V1.1 qualification and migration | New schema migration/interruption tests, private scans, docs and traceability complete |

### 27.8 Work after V1.1

POST candidates: complete cosmetic blocking, selected browser extension
support, platform/synced passkeys, broader compositor/GPU support, ARM64,
Flatpak, optional encrypted sync, richer password-manager integrations, per-
profile proxies if feasible, advanced permission partitioning, more elaborate
workspace policies, typed action pipelines, automatic context routing beyond
V1's prompt/suggestion model, and packaged web-app launchers. Each begins with
its own scope/API/security review. Avoid native plugin ABI design until real
extensions demonstrate an unmet need that subprocesses cannot address.

## 28. Packaging, updates, and support

### 28.1 Distribution model

**PKG-001 — Primary package.** Provide an Arch PKGBUILD for `rustbrowser` and an
optional `rustbrowser-omarchy` integration package. A `-git` development package
may follow. Use system Qt dynamically linked as the primary model, with an
explicit supported Qt family and minimum patched build. Build in a clean Arch
environment and test the package after installation. System dependencies are
part of the support contract; the Rust executable alone is not the product.

**PKG-002 — Dependency manifest.** Record package names/versions for Qt base,
declarative/Quick, WebEngine, Wayland platform plugins, required QML modules,
CA trust, spellcheck dictionaries, portals, and media prerequisites. On
Hyprland the screen-sharing portal and the file-picker backend may be
different packages. Hyprland's documented portal does not itself provide a
file picker, so the qualified setup needs an appropriate additional backend.
[Hyprland portal documentation](https://wiki.hypr.land/Hypr-Ecosystem/xdg-desktop-portal-hyprland/)

**PKG-003 — Optional components.** Separate optional DRM, dictionary languages,
external tools, and compositor integrations from mandatory engine resources.
Feature absence is visible in diagnostics and documentation. Do not mark a
missing microphone, camera, or portal as a browser compile-time failure; report
the actual prerequisite and qualification impact.

**PKG-004 — Bundling decision.** If Arch's available Qt build cannot provide
required behavior/security in a timely manner, decide explicitly whether to
ship a maintained application-specific Qt package or change the support
baseline. Bundling brings resource deployment, patching, licensing, size, and
CI responsibilities. AppImage/Flatpak are not automatic solutions to engine
maintenance and are POST unless separately resourced.

### 28.2 Update and engine policy

**UPDATE-001 — Engine version tracking.** At startup compare runtime engine
build against the release's qualified set and known-bad ranges. Exact known-bad
security/build cases show a prominent actionable update notice and may block
the affected feature. An untested newer patch is labeled unqualified until
tested, not automatically called vulnerable. A user can inspect true Chromium
base and security-patch versions because backported fixes make base version
alone insufficient. Qt describes its patch/backport model in its overview.
[Qt engine maintenance](https://doc.qt.io/qt-6/qtwebengine-overview.html)

**UPDATE-002 — Freshness.** Revalidate engine/package advisories for every
release, not just compare with this document's September 2026 versions. A
release must not knowingly include an applicable unmitigated critical engine
vulnerability. Monitor Rust dependencies, Qt, Chromium security notes, and
supported distribution packages. Rust dependency scanners do not cover the
embedded Chromium attack surface.

**UPDATE-003 — Operational targets.** Establish these initial maintenance
targets once maintainers and release infrastructure exist: triage critical
security reports within one working day; evaluate upstream critical fixes as
soon as a usable package is available; target a tested update within 72 hours
of that availability. These are proposed project service targets, not a claim
that an unstaffed project already provides an SLA. Publish current support
status and any inability to meet a target rather than leave stale security
claims standing.

**UPDATE-004 — Package manager authority.** The browser may display local
version/advisory information and documentation. It does not run pacman/yay,
alter repositories, downgrade system Qt, or replace its executable silently.
Omarchy integration must not freeze the entire system at an obsolete Qt patch.
Use the normal package manager and a qualified update workflow.

**UPDATE-005 — Data compatibility.** Rust DB/session/config migrations are
versioned and tested. Engine profile formats may not support downgrades;
application rollback must not claim to roll back Chromium storage safely.
Document supported downgrade boundaries and backups. Do not repeatedly launch
older engine versions against upgraded profile data as a troubleshooting trick.

### 28.3 Release artifacts and support

**PKG-005 — Artifact set.** A stable release includes source tag/archive,
Cargo.lock, build manifest, checksums, package recipe, installed-file manifest,
dependency/license inventory, user/developer docs, changelog, capability and
compatibility report, migration notes, and known issues. Signed artifacts or
repository signatures use an explicitly managed project identity, not an
invented maintainer key. Debug symbols are separately available where feasible.

**SUPPORT-001 — Issue reporting.** Templates request app/engine versions,
compositor/GPU/portal details, reproducible steps, expected/actual behavior,
whether a temporary profile reproduces it, and sanitized diagnostics. Security
reports have a private route established before public V1. Do not ask users to
upload complete profiles, cookies, or unredacted logs.

**SUPPORT-002 — Triage categories.** Classify core behavior, Qt adapter,
upstream engine, desktop/portal, graphics driver, packaging, site policy, and
user configuration separately. A workaround is documented with scope and
tradeoffs. Escalate to the relevant upstream with a minimal reproduction when
authorized; posting external issues/messages is a separate action from local
diagnosis.

## 29. Open-source maintenance and provenance

**OSS-001 — License decision.** Recommended initial project license is
GPL-3.0-or-later, aligned with the inspiration project's open-source approach.
The owner must select the actual license before publication; this specification
does not license future source by implication. If reusing qutebrowser code,
preserve its notices and compatible obligations. A clean implementation may
learn from documented behavior without copying code or branding. Record the
license decision in ADR-010 and create the real LICENSE file at that point.

**OSS-002 — Dependency notices.** Track licenses and source provenance for
Qt, Chromium, Rust crates, fonts/icons, dictionaries, blocklists, and scripts.
QtWebEngine includes multiple upstream licensing obligations; packaging must
preserve the notices and any corresponding-source/relinking materials required
by the selected distribution model. Codec and DRM rights are separate from
the application's own source license.
[QtWebEngine licensing](https://doc.qt.io/qt-6/qtwebengine-licensing.html)

**OSS-003 — Contribution process.** Publish CONTRIBUTING with build/test
requirements, module ownership, code style, how to add a command/setting,
required tests, FFI safety expectations, and how to reproduce Wayland bugs.
Maintain a code of conduct and security policy before public collaboration.
PRs describe observable behavior and tests, with screenshots only when useful.
Do not require contributors to expose a personal Google account to ordinary CI.

**OSS-004 — Maintenance discipline.** Keep one primary branch and release tags;
use semantic versioning for the public CLI/config/IPC promises after V1. IPC
major versions negotiate compatibility. Deprecations carry migration guidance.
Avoid speculative framework extraction, engine-generalization layers, and
feature flags without owners. Track the actual maintainers and support window
before promising users a durable release cadence.

**OSS-005 — Provenance and branding.** Keep a source-inspiration note for
qutebrowser and any reused snippets/assets. RustBrowser is a working name;
check final namespace/name availability before release and choose owned
desktop/repository identifiers. Do not represent the project as an official
qutebrowser, Google, Hyprland, or Omarchy product without authorization.

## 30. Decision gates and risk register

### 30.1 Required architecture decisions

ADRs are short, evidence-backed decisions with context, alternatives,
chosen option, consequences, test links, and reconsideration trigger.

| ADR | Decision | Default direction | Must be closed by |
| --- | --- | --- | --- |
| ADR-001 | Qt family/patch, package configuration, Rust/bridge versions | Current maintained Qt 6, Cargo-first, CXX-Qt | M0 exit |
| ADR-002 | Quick adapter's public API coverage and view migration | Qt-owned windows and Quick views | M0 exit |
| ADR-003 | Permission authority and retained request lifetimes | Rust store with application-managed engine decisions | M0 exit |
| ADR-004 | Native screen/window capture path and portal mapping | Engine/portal/PipeWire supported path | M0 exit |
| ADR-005 | URL/origin normalization and matching | Engine authority, tested Rust boundary | Before persistent grants |
| ADR-006 | Profile/session durability and private exclusions | Profile SQLite + atomic safe-GET snapshots | Before M2 exit |
| ADR-007 | Hint geometry and trusted activation across frames | Native overlays + validated engine input | Before advanced hint completion |
| ADR-008 | Installed packaging and engine-update responsibility | Arch system Qt; explicit qualified baseline | Before M3 beta distribution |
| ADR-009 | Hyprland window identity and Omarchy provider versions | Optional adapters with explicit capability probes | Before M4 exit |
| ADR-010 | Project license, public name, app ID, release identity | Proposed GPL-3.0-or-later; owner choice | Before public publication |
| ADR-011 | Context/profile/window identity and shared action-subject model | Context above one profile; captured generational subjects | Before M1 signature registries stabilize |
| ADR-012 | Journey retention/schema and ephemeral-profile teardown proof | Bounded profile-local graph; memory-only disposable state | Before V1.1 implementation |

An agent can propose and substantiate these decisions. Owner-dependent naming,
licensing, external-account access, or scope changes remain explicitly pending
when authority is not already available. They should not block unrelated
implementation work.

### 30.2 Risks and responses

| Risk | Why it matters | Required response and decision threshold |
| --- | --- | --- |
| Google rejects login or an OAuth policy rejects the environment | The central work-service promise could fail despite Chromium ancestry | Test M0; reproduce on Qt reference; no security-bypass workaround; owner decides if unresolved |
| Engine capture cannot consume Hyprland's selected window stream | A custom Rust portal picker alone does not complete WebRTC integration | Trace real path in M0; engine/portal fix or explicit scope decision |
| CXX-Qt does not expose needed Quick APIs | Broad unsafe wrappers could undermine maintenance goals | Narrow public C++ shim; if private API needed, stop and revisit ADR-002 |
| Moving live WebEngineView breaks graphics/focus/lifetime | Naive tab detach can lose state or crash | M0 migration spike; no silent reload masquerading as move |
| Qt and Rust origin parsers disagree | Permissions/site policy could apply to the wrong site | Engine-derived origins, differential tests, conservative failure |
| Per-site feature is only profile-global upstream | Focus-based mutation can alter other tabs | Honest scope rejection; no simulated per-site setting |
| Renderer callback outlives a tab/profile | Use-after-free or wrong-tab mutation | Generational IDs, retained-handle rules, teardown tests |
| Request interception deadlocks main thread | Every page load may hang | Immutable bounded snapshot, no profile/UI calls during interception |
| Frozen/discarded tab contains active work | Calls or unsaved documents can break | Default discard off, explicit engine eligibility, safe exclusions |
| Primary package falls behind critical engine fixes | Rust application updates alone do not fix engine vulnerabilities | Track exact patched builds, supported-package policy, update ownership |
| GPU/driver-dependent failures | One successful desktop does not establish broad Wayland support | Explicit GPU matrix, reference comparisons, qualified software fallback |
| Omarchy changes file layout, hooks, or launcher | Hardcoded integration becomes fragile | Versioned adapters, path override, source probing, core independence |
| Contexts become a second, ambiguous profile system | Users could misunderstand or accidentally cross identity boundaries | One-profile invariant, distinct UI vocabulary, prompt-only routing, authentication-chain regression tests |
| Universal switcher focuses or exposes the wrong target | Same-PID windows and private items make a plausible match unsafe | Stable IDs, generation checks, exact compositor mapping, private exclusion and explicit opt-in |
| Site Doctor fixes a site by leaving unsafe state behind | Troubleshooting could silently weaken security or make behavior irreproducible | Bounded reversible experiment objects, denylisted mutations, navigation/crash cleanup, explicit durable conversion |
| Clean-link rule breaks a signed or application URL | A privacy convenience could corrupt login, payments, or shared work | Conservative allowlisted transforms, preview before navigation, preservation corpus, immediate original recovery |
| Journey graph becomes detailed surveillance data | Relationships can reveal intent even without DOM content | Profile-local bounded retention, sensitive URL exclusions, private memory-only storage, clearing/export controls |
| IPC/userscripts become accidental web privilege bridge | Remote content could reach local execution | No page-accessible native command bridge; typed inputs and injection tests |
| Private data leaks into history/recovery/diagnostics | Users rely on private profile separation | Memory-only stores, marker scans, explicit output exceptions |
| Testing requires real accounts/hardware | CI cannot prove all promised features | Authorized restricted/manual qualification; blocked is not pass |
| Too much feature scope before first usable version | Parity can obscure engine feasibility and core correctness | Vertical milestone slices and explicit POST backlog |
| Single-maintainer or unfunded engine maintenance | A browser needs continuing security work | Publish honest support window; do not promise unsupported longevity |

## 31. Definition of done and traceability

### 31.1 Per-feature completion

**DONE-001 — Feature acceptance.** A feature is complete only when its behavior
matches this specification, error/cancel paths are implemented, relevant
automated tests pass, required real-platform evidence exists, generated help
and configuration documentation match, and the traceability entry points to
the code/test/evidence. No TODO stub on a mandatory path, silent unsupported
fallback, ignored error, or unconditional test skip qualifies as complete.

**DONE-002 — Milestone acceptance.** Every milestone has a runnable installed or
documented build, updated capability/decision records, successful required
regression tests, and a concise list of remaining limitations. A milestone
demonstration uses the same source commit as its evidence, not a local patch
missing from the handoff.

**DONE-003 — Stable release acceptance.** V1 requires all of the following:

1. All V1 requirements in this document are verified or changed by an explicit
   accepted scope decision reflected in the document and release notes.
2. The installed primary Arch/Hyprland/Omarchy candidate passes native Wayland,
   keyboard/IME, file/portal, persistence, private-profile, and security tests.
3. Required Google scenarios, including real Meet screen and window sharing,
   pass on the exact candidate with dated authorized evidence.
4. A USB FIDO2 flow, accessibility chrome test, secondary-compositor smoke
   test, and supported GPU matrix have accurate results and stated limits.
5. Crash recovery, disk-full/corruption, migration, and 24-hour soak criteria
   pass without hidden data loss or mandatory quarantined failures.
6. Engine/dependency advisory review is current; runtime sandbox checks pass;
   true engine versions and active workarounds are reportable.
7. The package installs/upgrades cleanly, finds all runtime resources, and
   does not overwrite user configuration or change desktop defaults silently.
8. User/contributor/security docs, license/provenance, release identity,
   update responsibility, and supported-platform declaration are complete.
9. Contexts, the universal switcher, Site Ledger/Doctor, typed actions,
   binding discovery, and clean-link operations pass T-041–T-048 on the
   installed candidate, including their security and private-data assertions.
10. The project owner has authorized actual publication; technical readiness
   is reported independently of that external action.

### 31.2 Traceability file

**DONE-004 — Requirement records.** At M0 scaffolding, generate one row per
normative requirement ID in `docs/requirements.csv` or an equivalent structured
tracker with fields `id`, `title`, `milestone`, `status`, `implementation`,
`automated_tests`, `manual_evidence`, `issue`, `notes`. Never mark verified from
documentation existence alone. Derive IDs from the document and detect missing
or duplicate entries in `xtask check`.

**DONE-005 — V1.1 release acceptance.** V1.1 requires a supported V1 baseline,
all JOURNEY and EPROFILE requirements verified, T-049 and T-050 passing on the
installed primary candidate, successful upgrade/interruption tests for the new
schema, a private/ephemeral marker scan after normal close and crash, updated
accessibility and performance evidence for the journey view, and complete
release notes explaining the new retained relationship data. Publication still
requires the project owner's explicit authorization.

The group map below initializes milestones and test expectations; individual
rows remain authoritative when a group spans multiple milestones.

| Requirement families | Primary milestone | Principal tests/evidence |
| --- | --- | --- |
| SPEC, PROD, ARCH, BUILD | M0 onward | ADRs, clean build, scope and evidence review |
| STATE, ENGINE | M0/M1 | T-006–008, T-012, T-025, T-038, bridge tests |
| UI, INPUT, CMD, NAV | M1 | T-001–005, T-007, T-029, T-033, T-039 |
| DISC, LINK, ACTION | M1/M2 | T-004, T-023, T-046–048, generated registry/help review |
| HINT, PAGE | M1/M2 | T-008–010, T-023–024, frame fixtures |
| TAB, PROFILE, STORE, SESSION | M2 | T-006–008, T-011–015, T-032, T-038–040 |
| CTX, SWITCH, SITE | M2/M4 | T-041–045, exact-window/workspace and provenance evidence |
| JOURNEY, EPROFILE (V1.1) | V1.1 | T-049–050, migration, retention, and private marker scans |
| CONFIG, THEME | M1/M4 | T-005, T-026, T-036, schema/example validation |
| SEC, PERM, AUTH | M0/M3 | T-003–004, T-011, T-029–030, T-035, G-01–03/15/17 |
| FILE | M2/M3 | T-016–019, T-037–038, G-04/06 |
| COMPAT, MEDIA | M0/M3 | G-01–17, T-017–018, T-028, T-034 |
| NET | M2/M3 | T-020–021, filter corpus, security boundary review |
| CLI, IPC, SCRIPT, EDITOR | M1/M2 | T-004, T-012, T-022–024, protocol fixtures |
| WL, HYPR, OMA | M0/M4 | T-002, T-017–019, T-026–028, G-09–12 |
| A11Y | M1/M3 | T-002, T-033, contrast/IME/screen-reader records |
| DIAG, FAIL | M0 onward | T-011, T-014, T-025, T-034, T-038 |
| PERF, TEST | M1 onward | Benchmark artifacts, regression suite, T-040 |
| PLAN, PKG, UPDATE, SUPPORT, OSS, DONE | M0/M4 | Release/installed package/provenance/evidence review |

### 31.3 qutebrowser-inspired parity declaration

The initial user-facing parity statement must enumerate these outcomes rather
than claim complete qutebrowser compatibility:

| Area | V1 intent | Explicit distinction |
| --- | --- | --- |
| Modal keyboard browsing | Included | Guaranteed escape and tested web-editor focus |
| Commands/counts/chains | Included | Own typed grammar; no exact parser compatibility promise |
| Hints and rapid hints | Included | Documented closed-shadow/canvas/PDF limitations |
| Caret selection | Included for ordinary DOM text | Rich/canvas editors use normal page input |
| Completion/history/marks | Included | Profile-local indices and data-only imports |
| Tabs/windows/sessions/undo | Included | Safe URL restoration, not full serialized renderer state |
| Per-site preferences | Included for supported scopes | Engine-global settings not faked per site |
| Config scripting | Declarative TOML + external tools | Python config is not executed |
| External userscripts/editor | Included | Versioned JSON/argv contract, no full FIFO-protocol compatibility |
| JavaScript userscripts | Documented subset | No full Tampermonkey API promise |
| Ad blocking | Network request blocking | Cosmetic/scriptlet completeness deferred |
| Downloads/PDF/print/DevTools | Included | Qt capability mapping documented |
| Profiles/private browsing | Included | Explicit profile isolation and no cloud sync |
| Task grouping | Contexts included | Contexts select one profile plus windows/sessions/workspace; they are not cookie containers |
| Cross-object navigation | Universal switcher included | Tabs and compositor windows are equal typed targets; private excluded by default |
| Site troubleshooting | Ledger and reversible Doctor included | Explanations and bounded experiments, never an automatic insecure fix |
| Action extension | Typed built-in/userscript actions included | One registry across palette/hints/IPC; no embedded shell language |
| Link privacy | Explicit clean-link tools included | Normal navigation remains unchanged; transforms are previewed and versioned |
| Keyboard discoverability | Prefix overlay/map/explanation included | Generated from effective bindings, not a static cheatsheet |
| Navigation journey graph | V1.1 | URL relationship recovery, not serialized renderer or form state |
| Ephemeral isolated profiles | V1.1 | Destruction on final owner, distinct from named durable/private sessions |
| Qt 5/WebKit/X11 legacy support | Outside scope | Current qualified Qt 6 and native Wayland |
| Chrome extension ecosystem | Outside V1 | Reassess separately against actual Qt support |

### 31.4 Agent handoff template

An agent completing a work period leaves a factual handoff containing:

- Current application/specification commit or exact file state if Git is not
  available; milestone and work-item IDs completed.
- Actual build/run/test commands and their observed results.
- Updated requirement statuses and evidence locations.
- ADRs made, dependency versions qualified, and unresolved capability gaps.
- User-visible behavior available now, including precise limitations.
- Next dependency-ready work items and concrete blockers requiring external
  input, hardware, credentials, permissions, or a product decision.

Do not end a handoff with a generic claim that every feature works when only
the core tests ran. Scope implementation claims to the evidence collected.

## 32. Signature product direction

This section defines the product-specific layer that should make RustBrowser
more than a Rust reimplementation of qutebrowser. The earlier sections remain
authoritative for engine, storage, security, and desktop behavior. Terms here
must reuse those boundaries rather than create parallel implementations.

### 32.1 Contexts: task identity above security identity

A **profile** is the hard engine and browser-data boundary. A **context** is a
named way to gather work that already belongs to one profile: windows, saved
sessions, workspace intent, presentation accent, default opening behavior, and
route suggestions. Multiple contexts may use the same profile; one context may
not span profiles. This makes “Work”, “Research”, and “Personal” useful task
spaces without teaching users that a color or workspace is cookie isolation.

**CTX-001 — Model.** A context has a durable UUID, unique slug, display label,
one named durable normal profile, optional ordered session references, optional
Hyprland workspace selector, optional semantic accent, default open target,
route rules, created/updated timestamps, and generated saved membership. The
profile reference is required and immutable while the context has live windows;
changing it later is an explicit migration that reopens safe URLs and never
moves cookies or a live engine view.

**CTX-002 — Membership.** A normal browser window belongs to zero or one
context and retains exactly one profile. Its tabs inherit that membership. A
live tab may move to a window in another context only when both contexts use
the same profile; otherwise the operation is an explicitly named safe-URL
reopen with state-loss notice. Popup windows inherit the opener's context when
one exists. Private and V1.1 ephemeral windows are not durable context members.

**CTX-003 — Enter and save.** `context-enter` first focuses the most recently
used valid member window, including an exact compositor workspace activation
request when supported. If none exists, it restores the context's last valid
saved membership/session lazily or creates a window with its profile and
default page. `context-save` previews and atomically records eligible windows,
tab restore descriptors, selected tabs, and workspace intent. It excludes
private/ephemeral tabs, unsafe POST/auth callback descriptors, DevTools, and
transient prompts under the session rules in section 14.

**CTX-004 — Routes.** A route has stable ID, site pattern, target context,
integer priority, behavior, and allowed entry points. V1 behavior is `prompt`
or non-mutating `suggest`: it may intercept an explicit open, external desktop
URL, or typed initial URL before a navigation request exists. The preview shows
source/target profile and whether choosing the route will focus, open, or
reopen. Route matching uses CONFIG-006, has deterministic conflict handling,
and never derives a context from page title or page-controlled metadata.

**CTX-005 — Authentication guard.** Routing never automatically moves an
engine popup, redirect, same-document navigation, submitted form, WebAuthn or
permission request, or any tab participating in an OAuth/authentication chain.
A route whose context uses another profile may be offered before initial
navigation only; after navigation begins, the user must explicitly copy/reopen
a safe URL and accept loss of opener, form, and live authentication state. No
“smart” hostname rule may split a login chain to make routing appear seamless.

**CTX-006 — Workspace and appearance.** Workspace placement is an optional
intent enacted by the Hyprland adapter after exact window identification. A
denied, missing, ambiguous, or unsupported compositor request leaves the window
usable and reports the failure. Context accents affect browser chrome labels
and switcher identity only, pass contrast checks, and never modify web content
or substitute for a visible profile/private indicator.

**CTX-007 — Visibility and privacy.** Status chrome and window/switcher rows
show context and profile distinctly. Sessions, IPC results, and diagnostics use
stable IDs and omit private content under existing policy. Saved context state
contains only the browsing descriptors permitted for its profile. Deleting a
context never deletes its profile, engine data, history, bookmarks, or named
sessions; the preview lists membership and route references that will be
removed or detached.

**CTX-008 — Robustness.** Config validation rejects missing profiles, duplicate
names/IDs, cycles through session aliases, contradictory routes without a
unique priority, invalid workspace selectors, and attempts to bind a context
to a private/ephemeral profile. A missing saved session or unavailable
workspace is a recoverable per-item error. Concurrent context commands capture
their target/revision and cannot retarget because focus changed.

Target examples:

```text
context-create work --profile work --workspace 3
context-enter work
context-save
context-list
context-route add 'https://*.company.test/*' work
open --context personal -- https://photos.google.com/
```

### 32.2 Universal switcher

The universal switcher treats a browser spread across tabs and compositor
windows as one addressable system. It is a native, keyboard-first surface, not
a page overlay and not merely a tab list.

**SWITCH-001 — Sources.** V1 indexes live tabs, windows, contexts, commands,
bookmarks, quickmarks, profile-local history, sessions, downloads, and eligible
recently closed tabs. Each result has kind, stable object ID and generation
where live, primary label, redacted secondary label, profile, optional context,
known workspace/output, privacy kind, recency, match fields, and allowed action
IDs. Sources query asynchronously and publish revisioned batches; stale batches
cannot replace newer input.

**SWITCH-002 — Search and ranking.** Plain text tokenizes as Unicode-aware
case-folded terms. Optional generated scope filters select result kinds without
inventing a separate query language; the UI exposes them as chips/completions.
Ranking is deterministic: exact identifier/name, exact title token, prefix,
all-token match, then bounded current-context and recency boosts, with kind and
stable ID as tie-breakers. Current-context boosting never hides an exact result.
The default displays at most `switcher.max_results` and offers a clear route to
the full manager for larger stores.

**SWITCH-003 — Actions.** Selecting a result invokes its default typed action:
focus for a live tab/window, enter for a context, execute/open-help for a
command, open for a stored URL, load-preview for a session, show for a download,
and safe reopen for a closed descriptor. An action menu exposes only actions
valid for that subject and invocation source. Delete, clear, overwrite, profile
change, and cross-context reopen retain their normal previews and confirmations.

**SWITCH-004 — Exact focus.** Live results carry captured browser IDs rather
than titles or indices. Focusing a tab activates its containing window and tab;
the Hyprland adapter may first request the exact known workspace/window, then
Qt activation. If either layer denies activation, the result reports what is
visible and leaves selection available. A destroyed/generation-mismatched target
returns `E_STALE_TARGET` and refreshes results instead of focusing a plausible
replacement.

**SWITCH-005 — Privacy.** Private and ephemeral items are excluded by default
from ordinary invocation, persistent indexing, and IPC. An explicit switcher
opened inside a private/ephemeral window may search only that same transient
profile plus nonsensitive commands; it does not mix normal history or reveal
other private sessions. Enabling IPC private queries still requires the caller's
explicit `include_private` and never makes transient results persistent.

**SWITCH-006 — Accessibility and scale.** Results expose role, kind, position,
profile/context, selected state, and action names to assistive technology.
Keyboard, pointer, and screen-reader activation use the same target/action IDs.
With 100 live tabs and a 100,000-row history database, keystroke-to-first-results
targets p95 under 50 ms for in-memory sources and p95 under 150 ms for the merged
result on the reference environment; cancellation prevents backlog growth.

### 32.3 Site Ledger and Site Doctor

The Site Ledger answers “what is this browser actually doing for this site?”
Site Doctor builds on that evidence with safe experiments. It must be useful
for Google applications, where failures may come from permissions, capture,
codecs, GPU state, storage, content settings, scripts, blocking, or an upstream
engine limitation rather than one generic compatibility toggle.

**SITE-001 — Ledger snapshot.** A ledger snapshot is captured for one
tab/document/origin and includes the DIAG-004 facts, their value, provenance,
scope, apply time, active/pending state, capability status, and safe remediation
actions. Sources identify compiled default, user file/line, profile override,
site rule, generated/runtime override, permission store, blocklist revision,
compatibility-quirk ID, engine observation, or desktop capability probe. If the
engine cannot expose a fact, display `unknown` or `unavailable`, never a guessed
success value.

**SITE-002 — Blocker evidence.** Keep bounded per-tab/document counters and the
last 100 sanitized decisions for blocked, redirected, and allowed-by-exception
requests. Report rule/list ID, resource type, first-party classification, and
decision without retaining authorization query strings or headers. Detailed
records are memory-only and expire on document replacement; aggregate counts
may enter diagnostics without site identity.

**SITE-003 — Experiments.** V1 experiments are individually registered typed
actions: reload once with blocking bypassed for the current site; reload once
without browser/page userscripts; reload once using compiled-default settings
for supported site-scoped keys; or open the current safe URL in a fresh view of
the same profile. Each preview lists exact changes, expected lifetime, known
state loss, and which observation would support a diagnosis. Unsupported or
unsafe experiments are absent, not shown as toggles that do nothing.

**SITE-004 — Reversal.** An accepted experiment receives an ID, captured
tab/document/profile, prior values, start/expiry condition, and visible badge.
Undo restores only that experiment's still-owned temporary layer; it does not
overwrite a later user change. Reload/navigation invalidation, tab close,
profile shutdown, or process restart removes the experiment. Startup scans
generated state to prove no experiment was serialized as durable policy.

**SITE-005 — Durable fixes.** When an experiment establishes a likely cause,
Site Doctor may propose a normal site rule, blocker exception, permission reset,
or compatibility report. The proposal shows the exact configuration/store
mutation, provenance, security/compatibility effect, and test/reload behavior.
It uses the existing command and confirmation path. It never silently makes a
temporary success permanent or submits an upstream report.

**SITE-006 — Site report.** A user may copy/export a sanitized site report with
app/engine/platform versions, origin reduced to an opt-in host field, active
rule/quirk IDs, capability statuses, experiment results, and correlation IDs.
Preview applies SEC-004 redaction. Cookies, tokens, form text, DOM, full request
URLs, account identifiers, and another profile's facts are always excluded.

### 32.4 Typed actions and the intent router

Commands express language; actions express intent on a typed subject. One
action definition must power command forms, hint targets, the universal
switcher, native context menus, IPC, launcher integrations, and registered
userscripts. This prevents each surface from inventing different security and
targeting behavior.

**ACTION-001 — Definition.** Every action declares stable namespaced ID, subject
types, verb/label/description, typed parameter schema and defaults, allowed
invocation sources, privacy policy, effect class, confirmation rule, required
capabilities, availability predicate, executor, completion provider, and
examples. The registry validates uniqueness and generates presentation and
protocol metadata. UI labels are localized separately from stable IDs.

**ACTION-002 — Subjects.** Initial subject types are `url`, `link`, `tab`,
`window`, `context`, `selection`, `download`, `history-entry`, `bookmark`,
`quickmark`, `session`, and `command`. A subject is immutable captured data plus
an optional live generation token; it is never reparsed as command text. Live
actions fail stale rather than fall back to the current object. Stored URL
subjects follow safe URL and sensitive-output policy.

**ACTION-003 — Built-in actions.** V1 includes at least open/focus/copy/clean-
copy for URL-like subjects; open/send/download for links; focus/close/move/mute
for tabs; focus/move for windows; enter/save for contexts; copy/search/send for
selection; open/show/cancel/retry for downloads; open/delete for eligible stored
entries; and help/execute for commands. Availability is capability- and state-
checked before display and again before execution.

**ACTION-004 — Intent routing.** An action resolves to one existing command or
bounded effect path with the captured invocation context. There is no action
pipeline, implicit shell, command-text interpolation, or dynamic page-supplied
action registration in V1. Identical action ID and parameters have identical
validation and confirmation rules across UI, hints, IPC, and userscripts; the
source may further restrict authority but never expand it.

**ACTION-005 — Userscript actions.** A reviewed installed manifest may register
a namespaced action backed by that userscript as defined in SCRIPT-002. It can
request only manifest-declared subject fields and cannot replace built-ins,
grant permissions, suppress confirmations, select an executable dynamically,
or return an undeclared privileged action. Disabling/removing the script removes
the action cleanly from every generated surface.

**ACTION-006 — Audit and errors.** Mutating action invocations carry operation
and action IDs in redacted logs and structured results. Errors distinguish
invalid subject/parameters, stale target, denied source/privacy policy, missing
capability, confirmation required, script failure, and executor failure. Repeat
and macros follow INPUT-006 eligibility declared by the action.

**ACTION-007 — Examples.** These are target interfaces generated from the same
registry, not special parser branches:

```text
action link open --target tab-bg
action link send --to mpv
action selection search --engine ddg
action tab move --context research
action url copy --clean
```

**ACTION-008 — External targets.** A configured `action_targets.<name>` creates
a typed `send` destination backed by a fixed executable and direct argv as
defined in section 15. It is a registry entry, so availability, subject fields,
privacy, errors, cancellation, and detached lifetime are identical from hints,
the switcher, menus, commands, and IPC. It does not become browser-process code
and cannot return browser actions; use the stricter userscript protocol when a
result channel is required.

### 32.5 Navigation journey graph (V1.1)

**JOURNEY-001 — Graph.** V1.1 records profile-local navigation nodes and typed
edges so users can recover branches hidden by a linear back stack. Edge kinds
are `navigate`, `redirect`, `opener`, `popup`, `hint`, `session-restore`,
`reopen`, and `branch-after-back`. Back/forward moves update the current node;
a subsequent new committed navigation adds a branch rather than deleting the
prior relationship. Same-document changes may update a node or create a
`navigate` edge according to the history transition observed by the engine.

**JOURNEY-002 — Data boundary.** A durable node contains UUID, profile UUID,
safe committed URL, safe title, visit/transition timestamps, and optional
source command/action category. An edge contains source/target UUID, kind, and
time. Never store DOM, form values, POST bodies, screenshots, cookies, referrer
headers, renderer state, or page-script output. Apply the history exclusions
for credentials and sensitive callback URLs.

**JOURNEY-003 — Retention and clearing.** Default retention follows the
profile's history retention and is capped at 50,000 nodes and 100,000 edges;
oldest disconnected branches prune transactionally. A zero history-retention
setting disables durable journey recording. History clear by time/origin and
whole-profile deletion remove matching nodes and incident edges in the same
durable operation. Bookmarking does not exempt a journey node from clearing.

**JOURNEY-004 — View.** `journey` opens an accessible native view centered on
the current node, with both a compact chronological outline and an optional
graph layout. It distinguishes redirect chains, opener relationships, and
branches using text/icons as well as color. Search and expansion are bounded;
large graphs load neighborhoods incrementally. The view never loads page
previews or contacts recorded URLs merely to render.

**JOURNEY-005 — Recovery.** Reopening a node uses the safe restore rules: the
preview names profile/context target and any unavailable live relationship,
then performs a new GET in the chosen target. It cannot recreate POST, OAuth
opener, authentication, form, or renderer state. Missing/pruned nodes fail
stale without choosing a URL-shaped substitute. The new visit links back with
a `reopen` edge when durable recording is eligible.

**JOURNEY-006 — Privacy.** Private and ephemeral profiles keep a bounded graph
in memory only, clear it with their final owner, exclude it from ordinary IPC,
and never merge it into a normal profile. Session persistence contains only
the existing safe descriptors, not private journey topology. Export is an
explicit user-selected file operation with preview and private warning.

### 32.6 Ephemeral isolated profiles (V1.1)

**EPROFILE-001 — Meaning.** An ephemeral profile is a newly allocated off-the-
record engine profile plus memory-only Rust stores for a disposable task. It
shares no cookie jar, website storage, cache path, history, grants, journey
graph, sessions, completion history, or undo stack with any named or ordinary
private profile. It has a unique transient ID and clear visual label.

**EPROFILE-002 — Creation.** `open --ephemeral`, the ephemeral hint target, or
`profile-create --ephemeral` creates a fresh isolated profile unless an explicit
invocation token requests another window in the same still-live ephemeral
profile. It never inherits a named profile's remembered permission allows,
site-data exceptions, userscripts, or history. Safe global UI/theme and deny
policies may apply; browser page scripts/userscripts default off.

**EPROFILE-003 — Lifetime.** The ephemeral profile may own multiple windows,
but no headless persistent owner. Closing its final window runs ordinary
download/capture/prompt confirmation, then cancels remaining requests, stops
capture, destroys views before the engine profile, clears all Rust indices and
temporary files, and invalidates its ID. An explicit downloaded file or
clipboard copy remains user-owned external output and is disclosed as such.

**EPROFILE-004 — Actions and limits.** Live tab/window actions work within the
ephemeral profile. Moving a live view to a normal/private profile, saving a
session, durable permission grant, adding it to a context, or silently writing
history is forbidden. A user may explicitly reopen a safe URL in a named
profile or add a bookmark to a selected named profile after a destination and
privacy preview; this transfers only that URL/title data.

**EPROFILE-005 — Recovery and verification.** Ephemeral profiles do not survive
process restart and never appear in crash recovery. Unique-marker tests scan
all browser-owned config/data/state/cache/log/temporary locations after normal
close and crash. The browser accurately states that engine/OS swap, core dumps,
downloaded files, clipboard managers, sites, and network observers remain
outside this guarantee.

### 32.7 Deliberate product limits and release placement

**DIR-001 — Compositor-native composition.** Browser-owned split view is not a
signature feature. RustBrowser invests in exact window identity, cross-window
search, context/workspace intent, focus, movement, and restoration so Hyprland
can remain the tiling system. This avoids two overlapping window managers and
keeps page viewport, focus, capture, and permission ownership intelligible.

**DIR-002 — Explain before automate.** Context routes suggest before moving;
Site Doctor experiments before persisting; clean links preview before changed
navigation; the switcher shows the object/action before dispatch. Automation
may be added later only when its authority, undo, and failure behavior are at
least as clear as the explicit V1 flow.

**DIR-003 — One registry per concept.** Commands, actions, settings, bindings,
contexts/routes, compatibility quirks, and cleaning rules each have one typed
registry and generated documentation/test fixtures. Do not solve discoverability
by maintaining hand-written menus or cheat sheets that can drift from runtime.

| Release | Required product-direction scope |
| --- | --- |
| M1 architecture | Context-aware captured identities, typed action registry, switcher source protocol, active binding discovery, cleaning-rule core |
| V1 user-facing | Contexts, universal switcher, Site Ledger/Doctor, typed built-in/userscript actions, clean-link operations, binding map/prefix guidance |
| V1.1 | Navigation journey graph and ephemeral isolated profiles |
| POST candidates | Packaged web-app windows, richer explicitly authorized routing, typed multi-action pipelines, reconsidered split view only for proven unmet needs |

## 33. Sources and verification notes

The following primary sources informed the engine and platform constraints.
They were consulted on 2026-09-14. Upstream pages and branches can change;
implementation must record the exact versions/commits it qualifies. Most of
this document defines new product behavior, not existing upstream guarantees.

| Source | Used for |
| --- | --- |
| [qutebrowser repository](https://github.com/qutebrowser/qutebrowser) | Product philosophy, source organization, project/license context |
| [qutebrowser contribution guide](https://qutebrowser.org/doc/contributing.html) | Commands, testing, development practices |
| [qutebrowser configuration guide](https://qutebrowser.org/doc/help/configuring.html) | Separation of user config and generated state; per-site inspiration |
| [qutebrowser userscripts](https://qutebrowser.org/doc/userscripts.html) | External executable integration inspiration |
| [qutebrowser changelog](https://github.com/qutebrowser/qutebrowser/blob/main/doc/changelog.asciidoc) | Engine/site regressions and legacy support context |
| [Arc Spaces](https://resources.arc.net/hc/en-us/articles/19228064149143-Spaces-Distinct-Browsing-Areas) | Comparative task-space vocabulary; not an API or compatibility dependency |
| [Firefox Containers](https://support.mozilla.org/en-US/kb/containers) | Comparative cookie-container boundary informing the explicit context/profile distinction |
| [Firefox profile management](https://support.mozilla.org/en-US/kb/profile-management) | Comparative full-profile boundary and user-facing identity model |
| [QtWebEngine overview](https://doc.qt.io/qt-6/qtwebengine-overview.html) | Qt/Chromium ownership, process model, embedding, patch model |
| [QtWebEngine features](https://doc.qt.io/qt-6/qtwebengine-features.html) | Media, PDF, lifecycle, notifications, codec dependencies |
| [WebEngineView QML API](https://doc.qt.io/qt-6/qml-qtwebengine-webengineview.html) | View operations, callback surface, popup adoption, frame access |
| [QQuickWebEngineProfile](https://doc.qt.io/qt-6/qquickwebengineprofile.html) | Public Quick profile APIs, interception, permission/push policy |
| [QWebEnginePermission](https://doc.qt.io/qt-6/qwebenginepermission.html) | Exact-origin scope and persistent/non-persistent permission types |
| [QWebEngineWebAuthUxRequest](https://doc.qt.io/qt-6/qwebenginewebauthuxrequest.html) | Authentication UX request states |
| [QWebEngineUrlRequestInterceptor](https://doc.qt.io/qt-6/qwebengineurlrequestinterceptor.html) | Synchronous request interception and blocking cautions |
| [QWebEngineUrlScheme](https://doc.qt.io/qt-6/qwebengineurlscheme.html) | Internal scheme registration and flags |
| [QtWebEngine platform notes](https://doc.qt.io/qt-6/qtwebengine-platform-notes.html) | Build/runtime platform constraints |
| [QtWebEngine deployment](https://doc.qt.io/qt-6/qtwebengine-deploying.html) | Helpers, resources, locales, and installed runtime requirements |
| [QtWebEngine licensing](https://doc.qt.io/qt-6/qtwebengine-licensing.html) | Qt and Chromium component licensing obligations |
| [Qt and Wayland](https://doc.qt.io/qt-6/wayland-and-qt.html) | Native client backend and compositor ownership |
| [CXX-Qt book](https://kdab.github.io/cxx-qt/book/) | Rust/Qt interoperability model |
| [CXX-Qt Cargo build guide](https://kdab.github.io/cxx-qt/book/getting-started/4-cargo-executable.html) | Initial Cargo-first build choice |
| [CXX-Qt CMake guide](https://kdab.github.io/cxx-qt/book/getting-started/5-cmake-integration.html) | Alternative build integration if required |
| [ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) | Consent/session/PipeWire capture contract |
| [FileChooser portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html) | File-selection request/response contract |
| [Hyprland portal documentation](https://wiki.hypr.land/Hypr-Ecosystem/xdg-desktop-portal-hyprland/) | Screen-sharing versus file-picker backend distinction |
| [Hyprland IPC](https://wiki.hypr.land/IPC/) | Optional compositor request/event integration |
| [Omarchy theme setter, quattro](https://github.com/omacom/omarchy/blob/quattro/bin/omarchy-theme-set) | Current-state palette location and replacement behavior |
| [Omarchy color resolver, quattro](https://github.com/omacom/omarchy/blob/quattro/bin/omarchy-theme-color) | Palette keys/fallbacks and provider-version awareness |

The researched API surface supports pursuing this design. It does not yet
establish that the chosen Rust bridge, Qt build, portal backend, and GPU work
together for every required workflow. M0 and the dated qualification records
are the mechanism for resolving that uncertainty before making release claims.
