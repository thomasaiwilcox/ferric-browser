# M2-10 health and diagnostics evidence

Date: 2026-09-20  
Status: in-progress

## Task card

- Requirements: ENGINE-005, DIAG-001, and DIAG-002.
- Files: `crates/browser-engine-qt/src/diagnostics.rs`,
  `crates/browser-engine-qt/src/lib.rs`, and `crates/rustbrowser/src/main.rs`.
- Observable result: the display-free CLI, `diagnostics.get` IPC method, and
  in-app `diagnostics` command use the same versioned, privacy-safe report
  with explicit capability states.

## Evidence

- The report contains build, runtime, engine, storage, capability,
  workaround, recent-error, protocol, and privacy sections. Standalone
  diagnostics reports an explicit empty audit state; a live `diagnostics.get`
  response supplies bounded recent failure categories and correlations.
- Read-only profile database inspection is deterministic and bounded to the
  lexically smallest 128 UUID-named profile directories; the report exposes
  the limit and whether additional valid profiles were omitted.
- On package-manager hosts, a bounded read-only probe reports the installed
  QtWebEngine package version (`pacman`, `dpkg-query`, or `rpm`); only the
  parsed version token is retained. This package version is not treated as a
  Chromium security-patch assertion.
- The build section also reports the linked public QtWebEngine module version
  from `QtWebEngineCore`'s version header, plus the runtime Chromium base and
  security-patch values from QtWebEngine's public APIs. These values are
  reported with runtime provenance and are not themselves an advisory
  conclusion.
- Every capability fact carries a status and provenance. Compiled support is
  not treated as runtime qualification; GPU decode, codecs, DRM, and WebAuthn
  remain explicitly unprobed or unknown. On Linux, the report performs bounded
  `/proc` reads and reports seccomp plus no-new-privileges for the current
  application process and, when present, direct QtWebEngine renderer-helper
  children. Helper results are conservative: no observed helper is distinct
  from an observed helper without a uniform sandbox posture, and PIDs/paths
  are never emitted. The report now performs
  bounded read-only user-D-Bus portal introspection, a Qt runtime-version
  probe, a PipeWire core probe, and a bounded Hunspell dictionary inventory,
  reporting FileChooser, ScreenCast, OpenURI, Notification, Qt, PipeWire, and
  sanitized dictionary identifiers independently. Dictionary paths are never
  emitted; a missing executable or empty inventory is reported as unavailable
  rather than inferred.
- `target/debug/rustbrowser diagnostics --format json` uses the running
  instance's shared `diagnostics.get` report when available and otherwise works
  without starting a GUI. Both paths emit no browsing URL, file URL, credential
  path, cookie, account, private-session, or page-console data.
- On 2026-09-20, the standalone command emitted a 19,937-byte JSON report;
  an independent scan found no HTTP(S), file, home-directory, or temporary
  paths, and the report retained explicit `available`, `unavailable`,
  `not-tested`, and `unknown` statuses.
- A compiled, bounded compatibility-registry manifest is validated and
  reported as reviewed data with schema/version, entry count, engine probe
  state, and active IDs. The shipped registry is intentionally empty until a
  reproducible workaround has a regression fixture and review condition.
- The same report classifies browser-owned maintenance traffic: telemetry,
  automatic crash upload, remote suggestions, history sync, push opt-in, and
  blocklist refresh policy. It does not enumerate page URLs or network
  contents.
- A live `diagnostics.get` report replaces the standalone network-blocking
  placeholder with the active profile's enabled/disabled state and bounded
  loaded/skipped-list and compiled-rule counts. It does not expose list bodies,
  source URLs, or request data; standalone diagnostics remains explicitly
  unprobed because it has no profile owner.
- `cargo test -p browser-engine-qt -p rustbrowser --locked --offline` passes,
  including schema/status and privacy assertions.
- The native diagnostics surface requires an explicit visible preview before
  enabling Save. Exports are private, atomic, local-only JSON writes that
  refuse relative paths and overwrites; the in-memory preview is cleared after
  a successful save. No automatic upload or retention of exported data is
  performed by the browser.

## Limitations

Advisory review based on the reported Chromium security-patch metadata, GPU
details, renderer-helper startup/fault qualification, codec/DRM/WebAuthn qualification, portal request/cancellation flows, dictionary
language configuration, and populated compatibility workarounds remain future
slices. The native diagnostics manager is implemented and shares this report
with CLI and IPC callers; exported files remain explicitly user-owned and are
not retained or uploaded by the browser.
Profile database health is now read-only inspected when a concrete profile
database is available; repair/export and full disk-failure qualification remain
future work.
