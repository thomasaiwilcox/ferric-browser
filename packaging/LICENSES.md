# Dependency and license inventory

This inventory covers the direct Rust and system-facing dependencies named by
the workspace and package recipe. Release automation must refresh it against
the exact locked graph and installed package metadata before publication.

| Component | Role | License / notice source |
| --- | --- | --- |
| Rust standard library | application runtime | Rust project Apache-2.0/MIT notice |
| CXX / CXX-Qt | Rust/Qt bridge | crates.io package license metadata |
| serde / serde_json / toml | typed configuration and protocol data | crates.io package license metadata |
| rusqlite / SQLite | durable profile storage | crates.io and SQLite license notices |
| uuid | opaque runtime identities | crates.io package license metadata |
| adblock-rust | maintained ABP/EasyList-compatible network filter | MPL-2.0 notice from the `adblock` crate |
| Qt 6 / QtWebEngine | desktop and browser engine | installed Qt distribution license notices |
| Wayland / XDG portals / PipeWire | optional desktop integration | installed distribution package notices |
| libc | bounded Unix process, filesystem, and socket integration | MIT OR Apache-2.0 crate notice |
| CXX-Qt build/runtime crates | generated Qt bridge and Qt type support | MIT OR Apache-2.0 crate notices |

Ferric Browser's workspace license declaration is `GPL-3.0-or-later`; the
complete application license text is shipped as `LICENSE`.
This file is an inventory boundary, not a substitute for shipping the license
texts required by the selected distribution and dependency packages.
packaging/license-policy.toml is the bounded machine-readable companion:
cargo xtask check verifies that each named package is present in the locked
Cargo graph and that its reviewed license/provenance fields are non-empty.
