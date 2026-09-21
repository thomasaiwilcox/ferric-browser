# M0-117: reviewed compatibility registry

Ferric Browser now ships a compiled, data-only compatibility registry. Each entry
has a stable ID, host pattern, engine-version range, symptom, supported change,
upstream issue, regression fixture, date added, review/removal condition, and
enabled state. Registry validation bounds the entry count, rejects duplicate or
unsafe IDs and site patterns, checks version ordering and dates, and refuses
malformed reviewed data.

The shipped registry is intentionally empty until a reproducible engine/site
regression has a tested, reversible workaround. No unsigned network data can
add or modify a workaround. Diagnostics exposes registry health, version, entry
count, engine-version probe state, and active IDs; site status and sanitized
site reports carry the same scoped active-ID information.

Evidence:

- `cargo test -p ferric-browser-engine-qt compatibility --locked --offline`
- `cargo test -p ferric-browser-engine-qt diagnostics --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  (expected timeout, no lingering process)
