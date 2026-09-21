# M0-118: browser maintenance traffic policy

Browser-owned network activity is now described explicitly in diagnostics and
active-profile diagnostics. Telemetry, automatic crash upload, history sync,
and remote suggestions are disabled or unavailable; remote suggestions are
rejected by configuration validation because no provider exists. Push remains
an explicit normal-profile opt-in through QtWebEngine, and private/ephemeral
profiles cannot enable it.

Blocklist refreshes are reported separately as bounded HTTPS maintenance
traffic: they require an ordinary profile with durable storage and configured
lists, support both the explicit command and the validated interval timer, and
retain last-known-good data. User-configured pages/scripts and site traffic are
identified as user/site traffic rather than silently presented as browser
maintenance. The report contains categories and policy state, never browsing
URLs, accounts, history, or request contents.

Evidence:

- `cargo test -p ferric-browser-config remote_suggestions --locked --offline`
- `cargo test -p ferric-browser-engine-qt maintenance --locked --offline`
- `cargo test -p ferric-browser-engine-qt diagnostics --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  (expected timeout, no lingering process)
