# M0-39 media session key boundary

Date: 2026-09-20  
Status: in-progress

## Task card

- Requirement: MEDIA-005.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, and the scoped
  `crates/ferric-browser-engine-qt/src/mpris_controller.{h,cpp}` bridge.
- Observable result: when `desktop.media_keys` is enabled, the supported
  hardware play/pause-toggle key is forwarded to the active WebEngine view's
  `ToggleMediaPlayPause` action. When a session D-Bus is available, one
  per-process MPRIS player advertises the active non-private HTTP(S) page and
  routes `org.mpris.MediaPlayer2.Player.PlayPause` through that same engine
  action. Metadata removes credentials, fragments, and obvious secret query
  keys; private profiles publish no title or URL metadata.

Unsupported media actions (next, previous, stop, and page/PiP metadata
surfaces) are not intercepted or advertised. The browser does not expose a
desktop-client quit operation or claim seek/track-list support.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
cargo build -p ferric-browser --locked
cargo xtask test mpris
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

Session-bus availability, desktop-player discovery, hardware key delivery, and
actual media playback remain manual qualification work. The static adapter
test verifies that the key path and MPRIS `PlayPause` path share the same
QtWebEngine action.

The `mpris` smoke uses a unique disposable normal-profile base directory and
the loopback fixture. It asserts that the live `Metadata` property contains
the retained query value while excluding the token and fragment, then calls
`PlayPause` and verifies that the browser remains alive. It also reads
`CanPlay` and requires `false` for the harmless non-media fixture: URL metadata
alone must not advertise a controllable player. Temporary-profile metadata
remains intentionally empty and is covered by the separate privacy policy path.

## Session-bus smoke

With a rebuilt `target/debug/ferric-browser` running in the current desktop
session, `gdbus introspect` showed one per-process
`org.mpris.MediaPlayer2.ferric-browser.instance<PID>` service at
`/org/mpris/MediaPlayer2`. The player interface exposed `PlayPause`, metadata,
and the bounded capability properties while omitting next/previous controls.
A live `gdbus ... org.mpris.MediaPlayer2.Player.PlayPause` call returned
successfully and the browser remained alive, including when the active page
was still `about:blank`; non-HTTP(S) pages are rejected before calling the
QtWebEngine action because that action is unsafe before a real document exists.
For an HTTP(S) fixture without recent page audio, `CanPlay` and `CanPause` are
false even though safe URL metadata is available.

An isolated normal-profile HTTP fixture also exported `xesam:url` with
credentials/fragment/secret-query filtering applied (for example,
`?token=secret&access_token=hidden&keep=value#fragment` became `?keep=value`).
OAuth-style token names are filtered alongside the generic secret names.
`xesam:title` remains the page-provided title and is not treated as a URL
security channel.
