# M0-116: bounded cosmetic filtering

Cached blocklists can now provide a conservative cosmetic-filtering subset when
`blocking.cosmetic_filtering` is enabled. Host-scoped `##` element-hiding rules
are parsed into bounded immutable policy data, host-scoped `#@#` exceptions are
honored, and the active document receives a sanitized style rule in the
application script world. Generic selectors, script filters, CSS declarations,
and other unrestricted ABP syntax remain ignored.

The injection path is shared by normal tabs, popup views, and secondary
windows. It replaces one per-document style element, limits active selectors,
and does not write page data or make network requests. No new persistence path
is introduced.

Evidence:

- `cargo test -p browser-engine-qt network_policy --locked --offline`
- `cargo check -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir`
  (expected timeout, no lingering process)
