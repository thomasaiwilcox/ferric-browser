# M0-159 fuzz targets

Latest local verification: 2026-09-20 — `cargo xtask test fuzz` completed
successfully, building all standalone targets and running each against its
checked-in corpus with 32 bounded iterations.

The fuzz lockfile was regenerated offline against the current Rust 1.98
compatible dependency graph before this run; the locked build is now
reproducible without updating the crates.io index.

The repository now contains a standalone cargo-fuzz package under `fuzz/`.
It has no Qt or display requirement for the core targets; the userscript-result
target intentionally compiles the Qt adapter's pure validation module without
starting Qt.

Compilation, bounded execution, and formatting evidence:

```text
cargo build --manifest-path fuzz/Cargo.toml --locked --offline
cargo fmt --manifest-path fuzz/Cargo.toml -- --check
cargo xtask test fuzz
```

The targets cover command chains and bounded built-in alias expansion, binding resolution, IPC frames and JSON
envelopes, URL/link cleaning, switcher ranking, site-rule validation, context
route parsing/matching, session restore plans, action-target schemas, hint
label assignment, manifest-gated userscript output, and reducer event
sequences with invariant checks. Inputs are bounded by
the production APIs; the corpus policy excludes credentials, cookies, account
data, and private URLs.

Each target has a small seed directory under `fuzz/corpus/`. Seeds are
deliberately synthetic and are intended to be minimized further by campaign
runs; they are not evidence of a completed fuzz campaign.

This is bounded corpus and contract coverage, not a completed release
qualification: long-running libFuzzer campaigns and additional minimized
regressions still need to be run on the qualified build environment.
