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
| QtWebEngine 6.11.2 profile interface | nullable download-source compatibility check | LGPL-3.0-only; unmodified interface, full license, NOTICE and source/hash provenance in `crates/ferric-browser-engine-qt/src/qt_compat`; license/NOTICE/provenance installed by the package recipe |
| freedesktop wayland-protocols 1.46 | owned Wayland export and toplevel tagging bindings | MIT; complete upstream notices retained in pinned XML and installed protocol provenance |
| pkg-config | build-time discovery of system libraries | MIT OR Apache-2.0 crate notice |
| cc | build-time C compilation of generated Wayland protocol implementations | MIT OR Apache-2.0 crate notice |
| Wayland client | required native window integration | installed distribution package notices |
| Qt Gui 6.11.2 private headers | qualified native lifecycle interface | installed Qt LGPL/GPL notices; interface use isolated in `native_wayland.cpp` |
| XDG portals / PipeWire | desktop services | installed distribution package notices |
| libc | bounded Unix process, filesystem, and socket integration | MIT OR Apache-2.0 crate notice |
| CXX-Qt build/runtime crates | generated Qt bridge and Qt type support | MIT OR Apache-2.0 crate notices |

Ferric Browser's workspace license declaration is `GPL-3.0-or-later`; the
complete application license text is shipped as `LICENSE`.
This file is an inventory boundary, not a substitute for shipping the license
texts required by the selected distribution and dependency packages.
packaging/license-policy.toml is the bounded machine-readable companion:
cargo xtask check verifies that each named package is present in the locked
Cargo graph and that its reviewed license/provenance fields are non-empty.

The separate `helpers/ferric-phone-auth` graph is checked independently. Its
pinned libwebauthn 0.10.0 source is LGPL-2.1-or-later; the exact original tree
hashes and direct-egress patch are in `vendor/UPSTREAM.json` and
`vendor/FERRIC_PATCH.md`. The package retains the helper/library source archive,
LGPL text and rebuild/relink instructions. QR rendering uses qrcode 0.14.1
(MIT OR Apache-2.0). The official PSL snapshot is MPL-2.0, with its complete
notice, license text, version, upstream commit and SHA-256 retained under
`data/` and installed with the package. Qt's bundled WebChannel client retains
its original copyright and SPDX notice in the embedded source resource.

Run helper `cargo deny --exclude-dev check licenses sources` for the production
graph; run `check_inputs.py` as well to verify exact test-only Git commits.
The upstream virtual-authenticator test dependency names a tag and is pinned by
its lockfile commit; the production source rule continues to require revisions.
