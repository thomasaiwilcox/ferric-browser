# Ferric phone-auth helper

This is a separate Cargo workspace and is not part of Ferric's root Cargo graph.
It validates and prepares bounded create/get requests, constructs the pinned
library's transient caBLE QR device, and contains a shared operation and
private-pipe serializer exercised by the supported virtual HID test backend.
Production still returns `transport_unavailable` after preparation: P08 must
admit a real route before QR progress, channel execution, or egress is enabled.
The test backend does not represent a phone, Bluetooth transport, or physical
authenticator.

## Pin and API inventory

`libwebauthn` is pinned to
`linux-credentials/libwebauthn@9c0e5f78ac445d35dca05170efbb0afefc5a89ae`.
The pinned manifest is package 0.10.0, edition 2021, Rust 1.88, and
LGPL-2.1-or-later (`COPYING` at that revision). Its default feature set is
empty. Optional features are `nfc-backend-pcsc`, `nfc-backend-libnfc`,
`reqwest-related-origins-source`, and `virt`. P01 enables none of them. Turning
off default features does not remove its ordinary HID, Bluetooth, D-Bus, TLS,
or WebSocket dependencies.

The reviewed v0.10.0 API prepares requests through
`MakeCredentialRequest::prepare` / `GetAssertionRequest::prepare` with a
`RequestOrigin` and `RequestSettings::origin = OriginValidation::Validate`.
Origin validation takes a `PublicSuffixList`; related-origin fetching can be
disabled with `RelatedOrigins::Disabled`. `SystemPublicSuffixList::auto()`
loads a system-maintained PSL and returns an error when neither supported file
exists. It probes `/usr/share/publicsuffix/public_suffix_list.dafsa` and
`/usr/share/publicsuffix/public_suffix_list.dat`, supplied by the distribution's
`publicsuffix-list-dafsa` or `publicsuffix-list` package. No PSL snapshot is
embedded in the crate. Later integration must fail closed if the runtime data is
missing.

For a future QR ceremony, the inspected API is
`CableQrCodeDevice::new_transient(QrCodeOperationHint, CableTransports)` followed
by `Device::channel(ChannelSettings::default())` and the `WebAuthn` ceremony
trait. `new_transient` does not attach a known-device store. The API offers
`CableTransports::CloudAssistedOnly` and `CloudAssistedOrLocal`; P01 does not
instantiate either. The former advertises the conventional cloud-assisted
WebSocket route, while the latter also advertises local BLE L2CAP and may be
ignored or rejected by older peers.

## Linux system and network inventory

The pinned normal dependency graph resolves in the upstream lock snapshot to
`dbus 0.9.11`, `hidapi 2.6.5`, `btleplug 0.11.8`, `bluer 0.17.4`,
`publicsuffix 2.3.0`, `tokio-tungstenite 0.26.2`, and `rustls 0.23.40`.
The libwebauthn manifest declares `hidapi` with `linux-static-hidraw` and
`bluer` with `l2cap`. Building the Linux helper therefore needs a C toolchain,
`pkg-config`, and the D-Bus development library (`libdbus-1`); no `sudo`, udev
rule change, or system service enablement is authorized. Runtime BLE requires
an accessible BlueZ D-Bus service and adapter. HID support uses the kernel's
hidraw devices and the running user's existing permissions. Missing devices or
permissions are unavailable conditions, never a reason to elevate privileges.

Hybrid transport also has a network-policy blocker for later integration:
the pinned tunnel calls `tokio_tungstenite::connect_async` directly, derives a
`wss://` tunnel host from the peer's hybrid advertisement, and follows up to
five redirects. The inspected API exposes no Ferric proxy-policy parameter for
that connection. P01 does not connect. A later integration must prove the
configured proxy/offline/restriction behavior or keep phone authentication
unavailable; page input must never choose a relay URL.

P01 did not probe the machine's BlueZ service, adapters, hidraw access, PSL
files, system TLS roots, proxy, or a phone. Their runtime availability is
unknown; no local capability or device-support claim is made.

## Toolchain, lock and licensing

The helper declares Rust 1.88 and pins `rust-toolchain.toml` to 1.88.0. The
Ferric root workspace remains at Rust 1.85. The included lock is based on the
`Cargo.lock` shipped at the pinned upstream revision, with its local
`libwebauthn` package recorded as the exact git source and the helper root
package added. Its lock was regenerated offline from the standalone helper
manifest with Cargo 1.98.0. That removed 157 unused records inherited from the
upstream workspace; the retained package identities and versions did not
change. The resulting 330 records are the helper root, 328 registry packages,
and the pinned `libwebauthn` Git package. The external
`evidence/P01-a1/DEPENDENCY_AUDIT.json` inventories every record and confirms
the exact Git revision. `cargo metadata --locked --offline` now succeeds; this
metadata check did not compile the helper.

Declared Rust floors are present for 246 of 330 records; 84 records do not
declare one. The highest declared floor is Rust 1.88.0 (`time-macros` 0.2.27).
The pinned 1.88.0 toolchain is not installed here, so T049 remains unqualified
and no build result is claimed. All 328 registry archive checksums were
compared with Cargo.lock and matched. The full per-package record, including
unavailable fields, is in the external audit receipt.

The helper source uses Ferric's GPL-3.0-or-later project license. The linked
`libwebauthn` package is LGPL-2.1-or-later and is statically linked by Rust; a
release must satisfy the LGPL notice, source, and relinking obligations. The
All 330 resolved records have a license metadata value. Some use legacy slash
separators and need SPDX review. The audit also found license identifiers
outside the current `deny.toml` allowlist, including alternatives in upstream
expressions, which require review before changing policy. This is metadata
inventory, not a complete legal compatibility, notice, or relinking review.
Keep unreviewed third-party licenses outside the allowlist. The helper's own
GPL-3.0-or-later project license is explicitly allowed by `deny.toml`.

## Protocol and operation behavior

Production admission requires HTTPS, a usable system Public Suffix List, and
the pinned library's strict origin/RP checks with related origins disabled.
The transient QR object uses nonpersistent settings and is never logged,
emitted, or displayed in this slice. Production does not open a caBLE channel,
network connection, or HID channel.

Pipe messages use version 1 `{v,id,kind,body}` envelopes. Status is a closed
enum (`submitted`, `completed`); a result or coarse error is terminal. Request
and terminal lines are limited to 1 MiB, progress lines to 32 KiB, stdout to
4 MiB, JSON depth to 32, native IDs to 128 ASCII bytes, and ceremonies to 180
seconds. Progress coalesces repeated or too-fast states and is capped at 10
accepted updates per second. Output is serialized into a bounded buffer before
the sink sees it. Secret-bearing values and stderr are not logged.

The operation core uses the pinned library's make/get calls and IDL
serializers. Get results preserve every assertion returned by the library;
the helper never selects the first assertion arbitrarily. An output failure
after device submission is uncertain and never causes an automatic retry,
provider reselection, or fallback.

### C05 extension conformance in this slice

| Category | Scope |
| --- | --- |
| Passed to the pinned parser and preserved by its response serializer | The P06 allowlisted fields represented by the pinned request/response types; this slice adds no independent extension-verifier claim. |
| Optional ignored with no success claim | None newly ignored by P07b. |
| Native-only | None newly classified by P07b. |
| Rejected | Unknown/unmapped fields, including `hmacGetSecret` because the pinned prepared request does not retain it. |
| Verified end to end here | None; P07b tests create/get envelopes, not extension semantics or independent RP verification. |

## Build and test

From this directory, with the assigned Rust 1.88 toolchain and the official
P06 fixture at `builds/P06-fixtures/public_suffix_list.dat`:

```sh
cargo +1.88.0 test --locked --offline
cargo +1.88.0 clippy --all-targets --locked --offline -- -D warnings
cargo +1.88.0 fmt --check
```

The suite includes strict P06 admission regressions, dynamic virtual-HID
create/get IDL envelopes, a disposable HOME/XDG/TMP observer for transient
metadata, and a lost-reply case after a real virtual-authenticator create.
Virtual HID is an in-process test fixture; it does not qualify required UV or
resident-key support, caBLE tunnel behavior, phone compatibility, or physical
success. No test opens a real radio or account.

P08 owns production proxy/egress admission and real caBLE routing. P17 owns
independent RP signature verification, native/application qualification, and
physical authenticator evidence. System PSL, BlueZ, permissions, proxy, and
phone availability remain runtime prerequisites, not claims from these tests.

## Build status

This helper does not enable the phone provider. The separate package remains
outside Ferric's root Cargo graph. Production request handling stays unavailable
until a later reviewed slice connects an admitted transport.

## Compiler verification — 29 October 2026

The complete helper workspace now builds and tests on its declared Rust 1.88.0
compiler. All 11 library tests and 4 binary tests passed, with zero failures or
ignored cases. `cargo +1.88.0 clippy --all-targets --locked -- -D warnings` also
passed. These results supersede the earlier compiler-unavailable notes above;
the dependency pin and advertised compiler floor are unchanged. They cover the
helper implementation and supported virtual test backend. Production phone
transport remains unavailable, and no physical phone qualification is implied.
