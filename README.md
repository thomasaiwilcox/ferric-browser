# RustBrowser

A proposed keyboard-driven browser with a Rust application core, QtWebEngine,
and native Wayland integration, designed primarily for Hyprland and Omarchy.
It treats profiles, task contexts, browser windows, and Wayland workspaces as
one commandable system, with unusually strong built-in explainability when a
site or desktop integration misbehaves.

The project is being implemented incrementally. The current build contains a
Qt-independent Rust core, typed reducer/effect model, command parser and
registry foundation, deterministic navigation resolver, and a graphical
QtWebEngine prototype with reducer-backed navigation callbacks. It is not
qualified for daily use.

Start with the [development specification](docs/DEVELOPMENT_SPEC.md). It defines
the product, architecture, behavioral contracts, implementation sequence,
compatibility gates, and evidence required before release. Commands and file
layouts described there are implemented as their corresponding slices land.

Current development checks:

```sh
cargo build --locked
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
cargo xtask test engine
cargo xtask test wayland
cargo xtask package arch
```

See [M0/M1 evidence](docs/testing/m0-01-m1-01.md),
[command/mode evidence](docs/testing/m1-02-m1-03.md),
[navigation/CLI evidence](docs/testing/m1-05-m1-09.md), and the
[capability matrix](docs/architecture/capabilities.md) for the exact current
scope and known gates.
