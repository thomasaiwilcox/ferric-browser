# Repository and environment bring-up

## Repository and clone

Repository is `https://github.com/thomasaiwilcox/ferric-browser`, branch `main`.
Canonical HTTPS clone: `https://github.com/thomasaiwilcox/ferric-browser.git`.
The supplied bootstrap performs a standard single-branch clone, then fetches main. It checks the exact owner/repository, refuses dirty/existing conflicting destinations and writes external run context. It does not assume a particular home directory or an already-mounted Codex checkout.

Manual equivalent for a genuinely new destination (do not run over an existing checkout):

```sh
git clone --single-branch --branch main https://github.com/thomasaiwilcox/ferric-browser.git ferric-browser
cd ferric-browser
git fetch origin main
git switch -c work/everyday-v1 origin/main
git rev-parse HEAD
git status --short
```

Use the bootstrap instead when the pack should be copied and evidence directories initialized automatically. Read the generated RUN_CONTEXT paths. Copying instructions is not permission to modify remote branches. Prefer ordinary non-root Linux; Git author identity must already be configured for local commits. No helper downloads, package install or arbitrary repository scripts run as part of clone.

## Verified build expectations at the authoring baseline

The root workspace declares Rust edition 2024 / `rust-version = "1.85"`; its CI uses stable. Do not confuse that declaration with a verified MSRV test result. `packaging/dependencies.toml` selects Qt 6.11.2 and matching private SDK headers for the existing download-source guard. Qt Widgets and KF6WindowSystem >=6.28 are now required. pkg-config, CMake, a C++17 toolchain, SQLite development libraries and normal Qt Quick/QML/WebEngine development/runtime dependencies are needed.

Follow the current `packaging/dependencies.toml`, `crates/ferric-browser-engine-qt/build.rs` and `.github/workflows/ci.yml`, not generic older install advice. On Arch/Omarchy, check installed package versions before using the system Qt stack. The dependency manifest lists qt6-base, qt6-declarative, qt6-wayland, qt6-webengine and kwindowsystem. Exact Qt patch/private headers must match. Do not upgrade the user's desktop or weaken the guard to make a build pass.

On Ubuntu or a disposable CI image, mirror the checked-out CI's qualified SDK and KWindowSystem build. It includes `.github/ci/build-window-system.py` and its pinned source manifest, a build-only Python venv, and scoped `CMAKE_PREFIX_PATH`, `PKG_CONFIG_PATH`, `LD_LIBRARY_PATH` and `QT_PLUGIN_PATH`. Use current source SHA/checksums; do not substitute the distribution's older KF6/Qt and claim equivalence. Package installation requires the host's existing authorization; if absent, record blocked_environment and continue pure work rather than using sudo or downloading an unreviewed toolchain.

Record `git --version`, `rustc -Vv`, `cargo -V`, `cmake --version`, `pkg-config --modversion Qt6Core Qt6WebEngineCore KF6WindowSystem` (as available), qmake/QMAKE path, OS/architecture, display backend, GPU/driver and compositor version. Do not record arbitrary environment variables, access tokens, browsing URLs or personal paths in committed reports.

## Build slots and artifact paths

Use per-worker target directories for pure Rust tasks only if the repository command supports them. The existing application harness defaults to `target/release/libferric_browser_engine_qt.a`; it does not automatically follow CARGO_TARGET_DIR. The native integration owner must explicitly supply `-DFERRIC_ENGINE_LIBRARY=/absolute/correct/library.a` to the new harness and record binary/source provenance. Existing xtask commands may assume root target paths; inspect them rather than moving paths blindly.

One native build owner at a time. Keep CARGO_BUILD_JOBS=2 or lower on constrained hardware until measured. Distinct worktrees must not concurrently overwrite the same static engine library or Qt runner. Production GPU behaviour must not be benchmarked with offscreen software settings.

## Fixture isolation and native environment

Use the checked-out fixture harness and temporary XDG roots/profile directories, never the daily browser profile. Tests are non-root and keep the Chromium sandbox enabled; a test environment requiring `--no-sandbox` is an environment problem to record, not a new production workaround. Offscreen tests can use the existing CI flags but cannot qualify native compositor focus or physical input.

Native work uses an isolated nested compositor and D-Bus session with a real test keyboard/input path where required. Existing portal fixture infrastructure now creates real Wayland resources; extend it rather than driving the user's live desktop. A nested Weston pass does not qualify Hyprland grouping; W14 needs actual supported Hyprland. A Qt-generated event is not automatically a compositor-delivered physical gesture.

HTTPS test relying party: bind loopback only and create a short-lived test CA/certificate trusted only by the disposable browser fixture. Do not disable certificate verification, add a global insecure origin exception or mutate the user's trust store. For physical phone tests, use a dedicated reachable test RP and test accounts/credentials with operator consent; retain no private keys or credential responses in evidence.

## Helper environment

The candidate libwebauthn at `9c0e5f78ac445d35dca05170efbb0afefc5a89ae` is package 0.10.0 and declares Rust 1.88. Create a separate `helpers/ferric-phone-auth/` workspace with its own lockfile and documented tested compiler. Keep the browser workspace floor unchanged. Verify transitive requirements; a declared floor is not proof the lock builds there.

Use the library's actual required system dependencies (D-Bus/BlueZ and compiled transport dependencies, public suffix data, TLS roots). `default-features = false` does not automatically eliminate its nonoptional HID/BLE dependencies. Pin and audit them; do not rewrite transport/crypto to avoid dependencies. Runtime missing Bluetooth/public suffix data/helper yields a useful unavailable state without hanging native authentication. Do not enable system services, grant privileges or relax udev rules on the user's machine without separate authorization.
