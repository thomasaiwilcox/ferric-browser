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
