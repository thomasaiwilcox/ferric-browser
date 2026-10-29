# Ferric phone-auth helper

This separate Rust 1.88 workspace implements strict request admission and one
transient phone ceremony over the pinned library's cloud-assisted caBLE route.
It is not connected to Ferric's production credential provider yet. Independent
boundary review, browser network-policy admission and physical qualification
remain required before the combined release can enable it.

## Private protocol

The parent sends one newline-terminated version-one request containing `id`,
`op` (`create` or `get`), the engine-captured HTTPS `origin`, WebAuthn JSON
`options`, and `network`. Only `network: {"mode":"direct"}` can start a
ceremony. The legacy empty policy keeps transport disabled; proxy, offline and
unknown policy forms fail closed. The parent must derive direct permission
from effective browser policy. Page content must never populate that policy.

Admission requires a usable Public Suffix List and the library's strict
origin/RP validation with related-origin fetching disabled. Localhost and IP
origins are ineligible. Unknown or unmapped request semantics are rejected.
`SystemPublicSuffixList::auto()` reads distribution-maintained data from
`/usr/share/publicsuffix/public_suffix_list.dafsa` or
`/usr/share/publicsuffix/public_suffix_list.dat`. If neither system format is
available, the helper reads `../share/ferric-browser/public_suffix_list.dat`
relative to its installed `bin/` directory. The package supplies an official,
versioned snapshot there; `data/provenance.json` records its source and hash.
Neither path can be selected by a page or request. Missing data is unavailable.

After admission the helper creates a nonpersistent `CableQrCodeDevice` with
`CloudAssistedOnly`, writes a bounded QR module matrix and flushes it before
opening the channel. Bluetooth proximity verification, the encrypted tunnel
and authenticator operations remain in the pinned library. No known-device
store is supplied. The helper never writes pairing or credential metadata.

Responses use `{v,id,kind,body}`. QR contains `width` and binary `modules`, not
a relay URL. Status is `submitted` or `completed`; result/error is terminal.
Request and terminal lines are limited to 1 MiB, progress lines to 32 KiB,
stdout to 4 MiB, JSON depth to 32, IDs to 128 ASCII bytes, and operations to
180 seconds. Each output line is flushed. A lost result never causes automatic
replay or provider fallback. Get results retain all assertions for an owned
account decision rather than silently choosing the first. Enumeration rejects
counts outside 1–16 and applies a cumulative 1 MiB incoming CTAP-payload budget
before decoding or queueing responses, including the initial GetInfo payload.
The authenticator's input allow-list limit is not an output assertion limit.

The parent cancels by terminating this one-shot process, using the existing
supervised termination/grace/kill path. The helper closes its channel after
operation completion. Request data, QR secrets, results and upstream panic
payloads are never written to stderr.

`process_request` remains an admission-only API for tests and diagnostics.
The binary uses `process_request_to_pipe`, which can run the explicitly
admitted direct route. Neither entry point grants browser-side authority.

## Dependency and egress patch

The base is `linux-credentials/libwebauthn@9c0e5f78ac445d35dca05170efbb0afefc5a89ae`,
package 0.10.0, edition 2021, LGPL-2.1-or-later. Exact source, original hashes,
notices and the small direct-egress patch are retained under `vendor/`.
`tools/check_vendor.py` verifies that only the documented files differ.
The patch also moves libwebauthn and Snow to maintained P-256 APIs; protocol
framing and cryptographic algorithms are preserved. Exact changes are recorded
in the vendor manifests and checked against upstream vectors.

The patch permits only `cable.ua5v.com` and `cable.auth.com`, TLS port 443,
no credentials/query/fragment in the URI, and no redirects. It resolves once,
rejects empty/oversized or special/private DNS answers, then connects to the
checked socket while retaining normal hostname/certificate verification.
Dynamic relay domains, direct BLE data transfer, proxies and remembered phones
are unsupported. Physical tests must establish compatibility of this narrower
route with the actual target devices.

Runtime prerequisites are system TLS roots, PSL data, an accessible BlueZ
service and adapter, and normal user permissions. Building requires Rust
1.88, a C toolchain, `pkg-config` and D-Bus development files. The dependency
graph also contains ordinary HID support. No privilege elevation, udev change
or service enablement is part of this implementation.

The root browser and this helper have separate locked Cargo graphs. The Arch recipe now retains helper source, notices and relinking materials.
Installation and combined release qualification remain pending. Do not infer those
from the root workspace's license checks. Upstream licensing and notices are
preserved in `vendor/COPYING`, `vendor/README.md` and the copied library.

### C05 extension conformance in this slice

| Category | Scope |
| --- | --- |
| Passed to the pinned parser and preserved by its response serializer | The P06 allowlisted fields represented by the pinned request/response types; this slice adds no independent extension-verifier claim. |
| Optional ignored with no success claim | None newly ignored by P07b. |
| Native-only | Enterprise attestation requires browser RP policy. `credBlob` and `getCredBlob` results are not preserved by the pinned library. The helper rejects these requests before transport and the shim delegates them to native WebAuthn. |
| Rejected | Unknown/unmapped fields, including `hmacGetSecret` because the pinned prepared request does not retain it. |
| Verified end to end here | None; P07b tests create/get envelopes, not extension semantics or independent RP verification. |

## Build and verification

From this directory:

```sh
python3 tools/check_vendor.py
python3 tools/check_inputs.py --release
cargo +1.88.0 test --locked --all-targets
cargo +1.88.0 clippy --locked --all-targets -- -D warnings
cargo +1.88.0 fmt --check
```

The admission/operation tests use the portable official PSL snapshot under `data/`. Virtual-HID tests exercise real
library create/get calls and serialization, not a phone, radio or physical
authenticator. Additional tests cover QR bounds, explicit policy admission,
relay/address exclusion, no transient metadata and uncertain-create replay
prevention. No automated test opens a real account or radio.

Physical iPhone, Android, native-key and independently verified RP results are
still pending. Native fallback activation, browser-supervised live routing and
release packaging must be qualified before enabling the phone provider.
