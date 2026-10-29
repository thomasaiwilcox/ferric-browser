> **Historical execution pack.** For current window-mode and phone-passkey implementation use [the active product plan](../WINDOW_PASSKEYS_PLAN.md). Hot tabs are deferred. The historical amendment/attempt machinery below does not control new work.

# Start here — Ferric Everyday Features implementation pack

**Pack:** everyday-v1 / 1.0.0 · **Prepared:** 24 October 2026
**Authoring baseline:** `e4aa108f9d63c99c3b3e977c2061d6d8dde48568` on `main`.
**Repository:** `https://github.com/thomasaiwilcox/ferric-browser`
**Clone:** `https://github.com/thomasaiwilcox/ferric-browser.git`
**Installed pack location:** `docs/implementation/everyday-v1/`.

This is a complete execution pack for a Codex coordinator and tightly scoped Luna workers. It is not an implementation of the browser features. The supplied proposal is retained verbatim in `reference/ORIGINAL_PROPOSAL.md`; its old source paths and already-completed prerequisites are superseded by `BASELINE.md` and the contracts in this pack.

## 1. Start in a new workspace

Use Python 3.11+, Git and an ordinary non-root user. From the unpacked archive:

```sh
python3 tools/validate_pack.py
python3 tools/bootstrap.py --workspace "$HOME/ferric-everyday-work"
```

The bootstrap clones the repository, fetches current `origin/main`, creates a **local** `work/everyday-v1` branch, copies this pack to the location above, and writes `RUN_CONTEXT.json`, the initial control state and evidence directories **outside** the product checkout. It does not install packages, compile, execute repository hooks, change the user’s running browser, push or reset anything. A nonempty destination is refused. On a later run, read the existing `RUN_CONTEXT.json`; do not re-run fresh-clone setup over it.

For a deliberately supplied existing **clean** checkout:

```sh
python3 tools/bootstrap.py --workspace /absolute/new-control-directory --repo /absolute/ferric-browser
```

The script checks the remote identity and cleanliness and creates a new local branch. It refuses a conflicting branch or installed pack. It never stashes, resets or deletes local changes. Existing checkouts are not silently fast-forwarded; S00 compares their HEAD with fetched `origin/main`. A divergent or outdated checkout needs an explicit coordinator decision/new worktree, not an automatic merge. Fresh clone is the normal path.

## 2. Read in this order

Coordinator reads `README.md`, this file, repository instructions (`AGENTS.md` in all applicable ancestors, if any, plus `CONTRIBUTING.md`), `BASELINE.md`, `ORCHESTRATOR.md`, `ENVIRONMENT.md`, then `control/plan.json`. Do not copy this pack's instructions over a repository `AGENTS.md`. Existing higher-priority safety/privacy rules remain binding.

Run `tools/preflight.py --repo /absolute/repo --output /absolute/workspace/evidence/preflight.json` and complete **S00**. Code-search indices can lag the branch: verify paths in the actual fetched checkout. Compare current HEAD against the authoring SHA above. The latest execution baseline is the fetched, reviewed source, not automatically the older proposal baseline or an unmerged feature branch.

Commit the imported pack separately before product work, after review. No commits are created by bootstrap. If git author identity is missing, report it; do not invent a human identity or attribution.

## 3. Execute, do not just plan

Use `python3 tools/ready.py --state /absolute/workspace/control/state.json` to find eligible work. Run one concrete slice per worker. `tools/make_packet.py SLICE_ID --output /absolute/workspace/packets/SLICE_ID.md` creates its bounded policy/contract packet. The worker still reads the named current source anchors. The coordinator controls assignments, locks and acceptance; workers never accept their own output.

The task graph covers all three features, production integration, packaging, native/device qualification and final review. Creating flags, mocks, a QR dialog, a singleton tab window or refusal paths alone is not completion. Where an environment or platform proof is unavailable, record the exact blocker, leave its capability gated, and continue unrelated executable work. Never label the overall work complete with required native/device gates missing.

## 4. Resume

Read `RUN_CONTEXT.json`, external `control/state.json`, `control/CHECKPOINT.md`, outstanding attempt records and the latest reviewed integration SHA. Check worktrees and owned processes before restarting jobs. Do not kill every Cargo/Qt process or delete generic caches. Use `RESOURCE_POLICY.md` and the dry-run cleanup tool.

For the A57 recovery, use the pack path selected by `RUN_CONTEXT.json` and validate the actual external state with that pack's `tools/validate_pack.py --state /absolute/workspace/control/state.json` before running `tools/ready.py` against the same state. A57 imports the separately user-authorized, independently qualified F02e2s repair. Its five failed numbered attempts remain historical; neither redispatch them nor infer a sixth attempt. The accepted source commit and pack-only reconciliation commit are distinct and both are recorded. Read the current checkpoint for the next eligible slice; downstream physical, security and performance qualification remains required.

## 5. Deliver

Provide local branch/SHA, per-feature implementation and qualification status, tests actually run, unavailable tests, dependency/packaging changes, measured hot-tab results and precise user journeys. No push, remote PR, merge, release, browser install, real-account credential registration or compositor configuration edit is authorised by this pack alone.
