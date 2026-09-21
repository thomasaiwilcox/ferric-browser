# Permission request boundary

This slice connects Qt WebEngine permission requests to the Rust-owned
configuration decision and a bounded grouped browser-chrome prompt queue.

## Implemented

- The normal and private primary profiles use
  `WebEngineProfile.AskEveryTime`, so Qt does not become a second persistent
  permission authority.
- Supported Qt request types are mapped to the configured Rust decisions for
  camera, microphone, screen capture, notifications, geolocation, clipboard,
  and local fonts.
- Secondary windows and popup views use the same configured/profile-local
  BrowserUi authority. Unresolved `ask` requests enter the same bounded native
  queue rather than bypassing the authority, and the interactive surface is
  hosted by the requesting window.
- `allow` and `deny` decisions are applied immediately to the exact engine
  request. `ask` requests show the requesting origin, capability, scope,
  private-profile state, and grouped identical-request count, with explicit
  Allow, Deny, and (where eligible) Allow for site actions.
- Resolution checks an exact configured rule first, then an unexpired
  profile-local durable rule, then a profile-session decision, then the global
  capability default. Private profiles do not consult normal-profile durable
  rules and their session decisions remain in memory only. In-memory session
  decisions use a typed key containing the effective profile UUID (or the
  fresh private-session UUID), normalized origin, permission type, and explicit
  profile-session lifetime.
- Unknown request types are denied. A pending prompt expires after 30 seconds,
  is denied when its tab/window starts a replacement navigation or closes, and
  remains in a FIFO queue while one prompt is interactive. The queue holds at
  most eight requests, groups identical origin/type requests, and applies a
  30-second origin/type cooldown when overflow is denied.
- Screen/window capture is always returned to the explicit per-request source
  chooser, even if a global capability setting contains `allow`; it has no
  durable or session grant. `permission-reset ORIGIN screen-capture` is a
  revocation boundary that reloads matching active views without creating a
  persistent permission row.
- Escape denies the current group; Enter is consumed without granting consent.
  Background primary tabs expose a `permission` badge in the tab label rather
  than selecting the tab or stealing page focus.
- Qt desktop-media requests now open a per-window source chooser with separate
  screen and window models. Selecting one source calls the public Qt
  `selectScreen`/`selectWindow` API for that request; Cancel, navigation, and
  window destruction call `cancel`, and no prior source is retained.
- Normal-profile rules are stored in the profile-local SQLite ledger. The
  `permissions` command and `permissions.query` IPC method list at most 1,000
  normalized exact-origin rules; `permission-reset` removes one validated
  origin/type pair, clears a matching in-memory session decision, and queues
  revocation of matching active-page use. Because Qt remains on
  `AskEveryTime`, there is no separate persistent Qt grant cache to reset; an
  matching registered primary/secondary views and tracked active capture views
  are reloaded so page-owned capture resources terminate through the engine
  lifecycle. Matching queued prompts are denied during the same revocation
  operation.

## Verification

```text
cargo fmt --all -- --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build -p ferric-browser --locked --offline
QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser \
  --basedir <temporary-directory> \
  -- file:///home/tom/Projects/Ferric Browser/docs/testing/fixtures/hints.html
# With a running instance:
ferric-browser query permissions --format json
ferric-browser command -- "permission-reset https://example.test notifications"
```

The clean Wayland startup exited only because of the intentional timeout and
reported no QML errors.

## Remaining permission work

This is not the complete permission service. Qt does not expose every
top-level-document/frame field on the pinned permission request surface, so
the browser retains the engine-provided exact origin and requesting view
context rather than claiming a stronger frame partition. System-audio/portal
qualification and permission-specific device indicators remain. Direct
stream-track termination is not exposed by the
pinned Qt Quick API; revocation therefore uses the documented active-tab
reload fallback, now coordinated across registered browser windows.
