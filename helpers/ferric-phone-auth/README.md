# Ferric phone-auth helper (protocol placeholder)

This is a separate Cargo workspace. It is not part of Ferric's root Cargo graph,
and it does not implement phone authentication. The executable reads one bounded
newline-delimited request and always returns a coarse error; valid `create` and
`get` requests receive `not_implemented`. Unknown versions, operations, fields,
malformed JSON and over-limit input fail closed. There is no QR generation,
Bluetooth scan, WebAuthn ceremony, network request or success response in P01.

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
package added. The external `evidence/P01-a1/DEPENDENCY_AUDIT.json` inventories
all 487 committed lock records, including upstream workspace-only records.
Cargo metadata for an evidence-only helper copy resolved 330 packages including
the helper; 157 records in the committed lock were not in that graph. A direct
`cargo metadata --locked` check refused because Cargo would rewrite the lock.
Therefore this lock is not yet verified for the helper and T049 remains
unqualified. Do not treat the copied upstream lock as a validated helper lock.

The declared Rust floors are present for 314 of 487 records; 173 records do not
declare one. The highest declared floor is Rust 1.88.0 (`time-macros` 0.2.27).
The pinned 1.88.0 toolchain is not installed here, so no tested floor or build
result is claimed. 481 registry archive checksums were compared with Cargo.lock
and all matched. The full per-package record, including unavailable fields, is
in the external audit receipt.

The helper source uses Ferric's GPL-3.0-or-later project license. The linked
`libwebauthn` package is LGPL-2.1-or-later and is statically linked by Rust; a
release must satisfy the LGPL notice, source, and relinking obligations. The
The lock inventory found SPDX license expressions for 486 records; one local
test-workspace package declares `license-file = "../COPYING"` but no SPDX
expression. Some upstream expressions use legacy slash separators, and some
license identifiers are outside the current `deny.toml` allowlist. The external
audit records these gaps. It is metadata inventory, not a complete legal
compatibility, notice, or relinking review. Keep unreviewed third-party
licenses outside the allowlist until they are reviewed. The helper's own
GPL-3.0-or-later project license is explicitly allowed by `deny.toml`.

## Build status

This placeholder does not enable the phone provider. After independent source
review and coordinator build registration, use the separate-helper commands in
`docs/implementation/everyday-v1/TESTING.md`. The pinned library's API inventory
and dependency metadata are source inspection only; neither the Cargo.lock nor
this helper has been built or tested. No phone provider is enabled.
