# Ferric Browser

> **Public pre-alpha:** Ferric is not ready for daily or security-sensitive
> browsing. Expect incomplete accessibility, cross-origin hint limitations,
> platform gaps, storage changes, and unqualified native interactions.

Ferric Browser is a keyboard-first, qutebrowser-inspired browser built with
Rust, Qt 6, and QtWebEngine. It keeps navigation, tabs, modes, commands,
profiles, sessions, and desktop workflows behind typed Rust boundaries while
using Qt for WebEngine and native Linux integration.

Ferric currently targets Linux on native Wayland, with Arch Linux and Omarchy
as the first packaging environments. It deliberately follows qutebrowser's
command names and keyboard ergonomics where practical, then documents Ferric
extensions rather than silently repurposing familiar behavior.

## Build and run

Install Rust 1.85 or newer, Qt 6.11 with QtWebEngine, Qt Declarative, and Qt
Wayland, plus a C++ toolchain. Then run:

```sh
cargo build --locked
cargo run --locked -p ferric-browser -- open https://example.org
```

Useful development checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
cargo xtask test engine
```

Existing pre-alpha data is never moved automatically. To copy and validate the
retired RustBrowser XDG roots into empty Ferric roots, run:

```sh
ferric-browser migrate --from-rustbrowser
```

The source remains untouched, and populated Ferric destinations are refused.

## Architecture

Input follows one direction:

```text
QML / Qt callback
  → typed bridge request
  → ferric-browser-runtime
  → core reducer and subsystem orchestration
  → typed runtime effects
  → Qt, WebEngine, desktop, or storage adapter
```

`ferric-browser-core` owns application invariants and exposes read-only state;
`ferric-browser-runtime` owns Qt-independent orchestration; the Qt crate owns
platform integration and effect execution. Command, action, binding, and
setting registries generate the user references under `docs/user/`.

See [architecture](docs/ARCHITECTURE.md), [security model](SECURITY.md),
[testing guide](docs/TESTING.md), [roadmap](docs/ROADMAP.md), and the full
[development specification](docs/DEVELOPMENT_SPEC.md).

## Security and support

QtWebEngine supplies the rendering engine, so Ferric's browser security also
depends on timely Qt/Chromium packages from the distribution. Use GitHub's
private vulnerability reporting rather than a public issue for suspected
security defects. See [SECURITY.md](SECURITY.md) and [support](docs/SUPPORT.md).

Ferric Browser is licensed under
[GPL-3.0-or-later](LICENSE). It is an independent project inspired by
qutebrowser; see [provenance](docs/PROVENANCE.md).
