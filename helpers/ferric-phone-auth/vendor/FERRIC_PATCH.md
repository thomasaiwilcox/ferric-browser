# Direct relay transport patch

The library is copied from the exact revision recorded in UPSTREAM.json. The
original per-file hashes and exact modified-file list are retained there.
Original licensing and notices are preserved. `check_vendor.py` checks both
libwebauthn and the Snow 0.10.0 crate copied from its checksum-verified registry
archive (`SNOW_UPSTREAM.json`).

The direct-egress connector and cable module declaration restrict routing.
The libwebauthn and Snow manifests select maintained p256 0.14, eliminating the
old production crypto-bigint 0.5 dependency. The source adaptations use the new
SEC1 point names and the library's OS-backed `Generate` API. A rand_core adapter
preserves the upstream deterministic PIN test RNG and is compiled only in tests.
Noise messages, curve parameters, cryptographic algorithms and protocol framing
are unchanged. These API adaptations need independent
review and the existing PIN/Noise/cable vectors must continue passing.

Independent review additionally found a device-controlled assertion enumeration
bound. The high-level WebAuthn operation now rejects zero or more than 16
assertions before fetching more; it never truncates account choice to the
unrelated input allow-list limit. The one-shot caBLE connection also limits
cumulative incoming CTAP payloads (including initial GetInfo) to 1 MiB before
queueing or parsing another response. Tests exercise hostile counts, legitimate
multiple resident assertions and the encrypted transport's queue boundary.

The first supported route uses the two relay names advertised by this pinned
library, TLS on port 443, no redirects, and validated public socket addresses.
DNS is resolved once, and TLS uses the checked socket with the original hostname
for normal certificate verification. Dynamic relay domains and proxies remain
unsupported. The browser must separately admit effective direct networking
before launching the helper; this patch cannot grant browser policy authority.

The address filter is deliberately conservative and excludes special ranges
from the [IANA IPv4 registry](https://www.iana.org/assignments/iana-ipv4-special-registry/)
and [IANA IPv6 registry](https://www.iana.org/assignments/iana-ipv6-special-registry/).
It additionally excludes IPv6 transition/protocol-assignment prefixes from this
initial route. Physical qualification must establish compatibility with the
actual target iPhone and Android versions. Dependency updates must refresh the
recorded original tree and review this patch explicitly.
