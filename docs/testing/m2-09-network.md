# M2-09 request-policy boundary evidence

Date: 2026-09-20  
Status: in-progress

## Task card

- Requirements: ENGINE-003 and the adapter portion of NET-001/NET-003.
- Files: `crates/ferric-browser-engine-qt/src/request_interceptor.h`,
  `crates/ferric-browser-engine-qt/src/request_interceptor.cpp`,
  `crates/ferric-browser-engine-qt/src/network_policy.rs`,
  `crates/ferric-browser-engine-qt/src/blocklist_updater.h`,
  `crates/ferric-browser-engine-qt/src/blocklist_updater.cpp`,
  `crates/ferric-browser-engine-qt/build.rs`, and `crates/ferric-browser-engine-qt/qml/Main.qml`.
- Observable result: every live Qt WebEngine profile can be attached to a
  public `QWebEngineUrlRequestInterceptor` whose callback consults only a
  bounded in-memory policy snapshot.

## Security and concurrency boundary

The interceptor copies a `std::shared_ptr<const PolicySnapshot>` under a short
read lock, then evaluates normalized exact-host or one-level wildcard-suffix
patterns without calling UI, profile, storage, network, or script APIs. Policy
replacement validates and bounds entries before swapping the immutable
snapshot. The adapter owns the interceptor, tracks attached profiles, and
detaches them during destruction.

## Evidence

- The adapter is compiled through CXX-Qt’s public C++ file path with
  `QtWebEngineCore` and `QtWebEngineQuick` linked.
- The registered `RequestInterceptor` QML type attaches to the primary and
  secondary profiles; popup views inherit the already-attached opener profile.
- `network_policy.rs` loads bounded cached `easylist.txt`/`easyprivacy.txt`
  sources, compiles them with the maintained Brave `adblock` Rust engine,
  preserves ABP exceptions and resource-type options, and leaves invalid or
  oversized sources out of the active snapshot.
- The deterministic Wayland harness loads `blocking.html` from the real
  Qt/WebEngine application and seeds an isolated cached list containing
  `||127.0.0.1^$image` and `@@||127.0.0.1^$script`. It uses one fixed loopback
  fixture listener, tears down the dedicated D-Bus/process group, disables
  host VFS mounts for the disposable run, and removes the private runtime
  directory. The browser starts on `about:blank` and receives the fixture URL
  through the normal instance IPC path after profile/interceptor bootstrap, so
  the assertion exercises the configured policy rather than an early startup
  navigation race.
- The live end-to-end assertion passes: `cargo xtask test blocking` loads the
  real Qt/WebEngine fixture, observes the blocked image and allowed script
  probes, and reports `blocked request suppressed and exception request
  allowed`. The harness waits for the exception fixture's one-shot request
  with a bounded deadline and remains fail-closed if either probe is absent.
- An isolated `blocklist-update` smoke fetched both configured HTTPS sources,
  installed `easylist.txt` and `easyprivacy.txt` atomically, wrote bounded
  validator metadata, and reloaded the policy; the status query reported
  99,378 blocked rules and 622 exception rules.
- A normal-profile QML timer reads the validated `blocking.update_interval_hours`
  setting, clamps it to 1–168 hours, and schedules the same owner-thread
  updater; private profiles and profiles without a storage base never start
  the timer, and an in-flight refresh is not duplicated.
- An explicit `https://` list source was fetched through the same owner path;
  its canonical URL received a bounded SHA-256-derived cache ID, installed
  successfully, and reported 51,613 blocked rules and 619 exception rules.
- When `blocking.cosmetic_filtering` is enabled, the same cached-list loader
  accepts bounded host-scoped `##` element-hiding selectors and `#@#`
  exceptions. The validated selectors are exposed to the normal tab, popup,
  and secondary-window QML paths, which install one per-document style element
  in the application script world. Generic selectors, scriptlets, unrestricted
  CSS, and other cosmetic syntax are ignored.
- Each profile interceptor keeps a bounded first-party-host counter map in
  memory. Primary tab delegates, the secondary window, and the status bar show
  the active tab's count; `blocking-status` also reports the active-site count
  without exposing paths or a browsing-history request log.
- The same status response exposes an on-demand, host-only explanation for the
  active site: resource host, first-party host, matched host rule, and a fixed
  reason string. No URL path, query, cookie, or request-body data is retained.
- Each profile interceptor also keeps a bounded, memory-only trail of the last
  100 sanitized blocked or allowed-by-exception decisions per first-party host.
  The Site Ledger exposes the active site's trail, including resource type,
  matched rule, and the bounded source list ID, and the QML document-load
  boundary clears that site's evidence. Source IDs are copied into the
  immutable Qt snapshot; they are never reconstructed from request URLs.
- `blocking-toggle --site` is routed through the captured active document and
  updates only that profile's exact-host bypass list; the interceptor compares
  the request's engine-provided first-party host, while global
  `blocking-toggle` changes only the blocker enabled state.
- A reviewed Site Doctor blocker result can be converted into a durable
  profile-local `blocking.bypass_sites` generated override. The exact host
  list is validated and bounded to 64 entries, saved atomically with the
  existing runtime-override path, and reloaded into every profile interceptor;
  permissions and TLS are unaffected.
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  reached the event loop with no QML/runtime diagnostics.
- `cargo build -p ferric-browser --locked --offline` passes.

## Limitations

The interceptor now records Qt-provided resource/navigation/initiator context
labels, avoids treating an unknown subresource initiator as the resource's
first-party site, and exposes an unknown-context count through
`blocking-status`. The separate `blocking.security_deny_hosts`
setting now provides that boundary: matching hosts are denied before ordinary
site bypass/exception handling, and the rules remain active when ordinary
network filtering is disabled. It is empty by default and independently
validated and bounded. `blocking-status` reports only the bounded count of
configured security-deny rules; it never returns the configured host values.

The updater currently supports the two named `easylist.to` HTTPS sources and
canonical explicit HTTPS sources through explicit or interval-triggered
refreshes. Full live EasyList/EasyPrivacy fixture evidence, exact tab ID
attribution from the shared profile callback, generic cosmetic rules,
scriptlets, broader live context qualification, and broader compatibility
remain next network-policy slices. The normal
`blocking-toggle --site` shortcut remains session-only; durable bypasses
require the Site Doctor proposal preview and confirmation path.

Loaded list IDs and bounded source metadata are also included in
`blocking-status` and `site.status`: accepted byte count, ETag presence, and
validated `Last-Modified` are shown when available. Missing or malformed
metadata does not invalidate the cached list.
