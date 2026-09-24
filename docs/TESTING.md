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
behavior rather than private implementation calls. Schema-upgrade tests use
explicit temporary roots and must preserve an existing database on every
failure. Clean-break root tests must prove that incompatible Ferric data is
refused until an explicit reset and that unrelated legacy roots remain
untouched. Performance comparisons use release builds and record the machine,
Qt/Chromium version, workload, and sample count.

Application-boundary tests must prove that configuration updates are atomic,
profile and context mutations enter through their typed methods, and every accepted
storage ticket reaches exactly one terminal effect even when a profile closes
or is replaced. Architecture checks reject durable-resource ownership and
storage implementation types in the Qt presentation state.
