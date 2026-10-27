# Current-code baseline and amendments

## Frozen source

Inspected Ferric `main`: `e4aa108f9d63c99c3b3e977c2061d6d8dde48568`. Earlier proposal: `af30f6fb889ba694bf9fd2d71bbab3779427882e`. Source/SDK claims are inspection findings, not executed test results. Exact inspected files, blob IDs and coverage are in `control/source_manifest.json`. The execution agent must fetch current `main`, then reconcile intervening changes rather than force the repository back to this SHA.

A container-side network clone was unavailable during authoring. Source was read through the connected GitHub API. No local build, source checkout execution, native compositor test or phone ceremony was performed. The bootstrap is for the user’s agent environment, not a claim that cloning succeeded here.

## Changes that invalidate old instructions

**A01 — Do not rebuild secondary-window hints.** `FerricBrowserWindow.qml` now owns a `FerricHintWorkspace`, `FerricPageSearch`, modal input scope and per-window appearance/profile state. The historical `hint_chrome_available: false` finding is obsolete. Check current complete wiring and remaining parity gaps in F05; extend the existing workspace only where a behavioural test fails.

**A02 — Controller paths moved.** Primary composition is `FerricPrimaryBrowserWindow.qml`. It instantiates `FerricWindowLifecycleController`, `FerricPageServices`, `FerricWindowPresentationController`, `FerricWindowRequestsController`, `FerricCommandEntryController` and `FerricRuntimeController`. Do not resurrect `FerricBrowserRuntimeChrome.qml`/`...Services.qml` merely because the original proposal names them. The inline `webViewComponent` is still a source anchor; extract or adapt once, preserving captured callbacks.

**A03 — Shared resources already exist.** Reuse `FerricProfileResourceRegistry.qml`, `FerricProfileResources.qml`, `profile_lifetime.{h,cpp}` and application `profile_session.rs`. The application layer already shares durable profile-session permissions, configuration and downloads. Do not create a competing profile cache keyed only by a display name. Prove first-window closure and ownership handoff rather than assuming either failure or complete correctness.

**A04 — Preserve captured engine requests.** `qml/scripts/EngineRequests.js` executes a request only after `request_target_current(tabId,generation,documentId)`. New work must use this identity-aware route, not add ad hoc tab-separated command strings or read the active tab when a callback returns.

**A05 — Current native build is broader.** Qt remains 6.11.2. Qt Widgets, pkg-config and KWindowSystem >=6.28 are required. The dependency manifest records a version-qualified native download-source guard requiring matching private SDK headers. Keep that guard and its qualification; new feature work is not permission to disable it or casually upgrade the SDK. CI already builds the public window-export dependency and runs portal/application suites.

**A06 — Foreign-toplevel export is not compositor identity.** Current file-chooser work uses KWaylandExtras window export. An xdg_foreign parent handle does not by itself establish a Hyprland client address or stable ID. W00 must prove an exact mapping on the installed backend.

**A07 — Helper MSRV is separate.** Inspected libwebauthn candidate `9c0e5f78ac445d35dca05170efbb0afefc5a89ae` declares package 0.10.0 and Rust 1.88. Ferric declares 1.85. Put the helper in `helpers/ferric-phone-auth/` with its own workspace, lockfile and declared toolchain floor; do not raise Ferric's advertised floor as incidental work. Test the resolved helper toolchain and record any higher transitive floor rather than pretending 1.88 alone guarantees the whole graph.

**A08 — Search is discovery, not baseline truth.** GitHub code search returned an earlier indexed snapshot during authoring. Every worker resolves the supplied anchor against the actual execution checkout before editing. The pinned manifest distinguishes full reads from excerpts.

## S00 reconciliation algorithm

1. Record remote, HEAD, fetched origin/main, worktree status and applicable instructions. Do not merge other agents' branches.
2. With a fresh clone use latest main. With an explicit existing checkout, compare divergence; preserve it and choose a clean additional worktree or record a branch blocker.
3. Run `git diff --name-status e4aa108f9d63c99c3b3e977c2061d6d8dde48568..HEAD` when the authoring object is available. If missing, fetch that exact object or compare manifest blob IDs; do not treat missing history as zero drift.
4. Resolve each source-manifest path and every slice anchor. Produce `source-map.execution.json` mapping historical/current/new paths, symbols and any already-implemented requirements.
5. Reconcile ownership, config keys, reserved worlds, engine request schema, build flags, public/private SDK glue and test registration. Update slice allowlists and locks in a reviewed plan amendment, not in worker discretion.
6. Freeze execution SHA and document contracts before implementation. Later upstream changes require another explicit integration review; no perpetual fetch/reset loop.

## Existing tests to preserve

Use the actual current CTest inventory, including page search, hint controls, open targets, navigation recovery, form reload, streaming history, tab/last-tab recovery, session/startup recovery, switcher/privacy, native portal chooser and download ownership. The current CI has explicit named suites; do not replace these with a new isolated happy-path test executable. `TESTING.md` gives exact baseline commands and how to register added suites.

## S00 execution reconciliation — 24 October 2026

The exact `origin` is `https://github.com/thomasaiwilcox/ferric-browser.git`. Fetched `origin/main` remains `e4aa108f9d63c99c3b3e977c2061d6d8dde48568`, matching both the authoring source and the current product tree. The older proposal object `af30f6fb889ba694bf9fd2d71bbab3779427882e` is available; its 555 changed paths confirm why the current-code amendments above must take precedence. The execution pack was imported in the separate local documentation commit `e4e907db673957d4b7b543812c20d876bd880248`. It contains no product-source delta. No new task-path or ownership amendment is needed.

`control/source-map.execution.json` resolves all 185 unique plan/manifest paths: 64 exist at the product baseline; absent paths are classified as pack artifacts, planned outputs or proposed deliverables, with no deleted source path silently treated as current. The manifest's missing blob for `tests/application/CMakeLists.txt` is resolved to `a77cb631ef6bf3c4d1dbed954d3cc2443e8be6c5`. The map records current file identities, proposal renames/deletions, ownership and test-registration limits.

Reconfirmed reuse decisions: secondary windows already have hints, page search, settings, modal input and per-window presentation; primary composition uses `FerricPrimaryBrowserWindow.qml` and existing controllers; the inline WebEngine page component remains to be extracted only through F02; shared profile resources and lifetime are owned by the existing registry/native lifetime plus application `ProfileSession`; delayed browser requests use the captured `tabId`/generation/document identity route. Existing WebAuthn remains Qt's `WebEngineWebAuthUxRequest` and native security-key prompt. Browser-owned scripts use `ApplicationWorld`; page-visible evaluation and installed user scripts use `MainWorld`. No phone credential bridge/helper exists in the product baseline, so P00 must establish authority before P03 adds one.

The host is Omarchy/Arch Linux x86_64 with Rust 1.98, CMake 4.4.4, Qt 6.12.0 and KWindowSystem 6.31.0. The production adapter still requires exactly QtWebEngine 6.11.2 and matching private headers, so the installed system Qt is not a valid native build SDK and the private download guard must remain intact. Weston, D-Bus and Hyprland executables are present; their presence does not qualify native behavior. Physical devices and GPU/driver were not probed. Btrfs reports about 122 GiB free (48% used), with metadata allocation 47% used; no root-repository `target/` or registered run build exists. No Ferric product build/test or native/device qualification was run in S00.

Source inspection found 65 direct application `add_test` registrations and three portal registration statements (one is generated from a CMake variable); this is not a generated CTest inventory or a pass. S01 must configure the production-backed harness, capture actual test discovery and establish reachable baseline results. Repository gates remain the commands in `TESTING.md` and the fetched CI; no suite or guard is waived.

## A53 — Durable ID is not actual profile-session identity

A52 attempt2 reached a ready Ephemeral runtime but its optional durable ProfileId was empty. ProfileSession already owns actual session resources through Arc/Weak lifetime; no public session identity projection exists. F02e2i supplies an opaque typed live identity and BrowserApplication accessor in two existing application files. The original source baseline and failed histories remain; only the reserved F02e2s attempt3 source-base rule is overridden to accepted prerequisite integration descended from eb6ff74. No adapter path or attempt budget is widened.


## A54 exact test-only repair

A54 preserves F02e2i failed source candidates1/2/3 and proposes only a pinned cfg-test correction to A3 ba665d9e; accepted product integration remains eb6ff74 until exact integrated source and actual focused/full application regressions are independently accepted.
