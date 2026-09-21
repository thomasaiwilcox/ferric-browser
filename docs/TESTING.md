# Testing Ferric Browser

Run the deterministic repository gate with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask check
```

Qt and QML qualification additionally uses `cargo xtask test engine`.
Native-Wayland journeys use `cargo xtask test wayland`; they require the system
QtWebEngine stack, Weston, D-Bus, and the relevant portal services. Packaging is
checked with `cargo xtask package arch`.

Tests should assert state, effects, typed errors, and externally observable
behavior rather than private implementation calls. Migration tests use
explicit temporary roots and must prove source preservation and overwrite
refusal. Performance comparisons use release builds and record the machine,
Qt/Chromium version, workload, and sample count.
