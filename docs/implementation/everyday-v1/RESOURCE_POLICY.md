# Resource ownership and safe cleanup

This project has large Rust/Qt build outputs. Resource pressure is a real work blocker, not a reason to delete unrelated data. Use one native build and initially one additional small Rust build maximum. Start native builds with two jobs or fewer on constrained hardware. Do not create a full native target directory for every worker.

## Before and during expensive work

Record filesystem free bytes and inodes, build-output locations and active build owners. Initial conservative admission floor: 12 GiB free and at least 10% of the filesystem, plus reasonable inode headroom. These are chosen operational limits, not measured Ferric requirements. Increase the floor when observed builds need more. On Btrfs, ordinary `df` does not establish metadata headroom; read available filesystem-usage information without elevation and record unknown if unavailable. Never run a balance, delete snapshots, change quotas or expand filesystems automatically.

If capacity falls below the accepted floor, stop admitting builds, let owned work checkpoint safely, record the limit and inspect only pack-owned artifacts. Running out of space can invalidate test results and truncated logs; do not mark them passed.

## Artifact ownership

The external workspace has `builds/`, `evidence/`, `packets/`, `control/`. Only direct children of `builds/` carrying an exact `.ferric-build-owner.json` for the current run are eligible for the supplied cleanup tool. Register build outputs before use; never retroactively mark an arbitrary directory as owned simply to delete it. Root-repository `target/` created by existing xtask is intentionally NOT auto-deleted by this tool; coordinator must inspect it and obtain an explicit narrowly scoped cleanup decision. No automatic Cargo global cache cleanup.

Record active builds in `control/builds.json` and use the external `control/build-active.lock` while a compiler/native test is running. Cleanup is forbidden while this marker exists or any registered build is active. Marker removal requires proving the owned process ended; stale chat activity is not proof.

Use `tools/cleanup.py --workspace /absolute/run` to preview. `--apply` requires the exact listed artifact names and matching run owner marker. The tool refuses symlinks, path escape, nonowned directories and unknown names. Source checkouts, .git, worktrees, controls/evidence, dependency caches, profiles, home and system temporary directories are not eligible.

A complete registered build directory may be removed after its owner finishes, logs/receipts and needed binaries have been retained, and downstream work no longer depends on it. Do not delete active targets, recursively follow symlinks, use `rm -rf /tmp/*`, `cargo clean` against unknown targets, `git clean -fdx`, or kill processes by generic name. Timeout supervisors may terminate only the process group they started; helper cancellation likewise owns only its helper.

## Checkpoints

Inspect resources after S01, after each lane's major integration, before H06/P17/W14, and at X05. Keep at most the needed native integration build and the active helper/pure builds. Retain enough raw benchmark/device evidence to reproduce conclusions; only deterministic disposable fixtures/builds are cleanup candidates. X05 proves a rebuild after owned cleanup without relying on stale binaries.

The pack never authorizes deleting Codex chat history, downloaded project repositories, another project's builds, the installed browser, saved credentials or snapshot data. A missing/ambiguous owner marker means leave the data alone.
