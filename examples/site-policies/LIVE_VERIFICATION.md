# Live example verification — 15 November 2026

Apple navigation, Reddit's current home-feed variant and YouTube's one-shot
player mapping passed the scoped checks below. **P13-10 remains incomplete:**
YouTube sidebar and Hint-context media checks are still open. The channel grid
passed the later scoped replay below. A confirmed Qt 6.11.2 corruption path is
now mitigated; the original live crash's cause has not been uniquely reproduced.

| Site | Observed variant | Actual result |
| --- | --- | --- |
| Apple UK (`https://www.apple.com/uk/`) | Signed out, HomeView hero, desktop Global navigation | Captured Mac through the guided builder; chose its unique selector, explicit top-document root, normalized name and link origin/path; reserved `m` in family `all`. Nonactivating preview reported Live. After full review and explicit application, `hint --target current all` then `m` navigated to `/uk/mac/`. |
| Reddit (`https://www.reddit.com/`) | Signed out; optional cookies rejected with owner authorization; home feed showing “Just another Saturday night in A+E” | Imported the unchanged native builder export from the earlier feed. Preview reported code `1` Live and `2`/`3` Missing. After review and application, code `2` matched nothing and did not navigate. Backspace then `1` followed the visible post's public title link. The earlier feed variant returned Incomplete and was not applied; that failure remains retained. |
| YouTube player (`https://www.youtube.com/watch`) | GB signed out; Reject all chosen with owner authorization; public Blender video | Built and reviewed `p` → `k`, applied it, and selected the workspace's one-shot passthrough test. Raw `k` first paused at 0:38. The actual virtual `p` then resumed: screenshots show the central play indicator and bottom pause bars; a later frame advanced to 3:33. Hint-context delivery was not tested. |
| YouTube grid (`https://www.youtube.com/@BlenderOfficial/videos`) | GB signed out, one PageDown, partial first/third rows and full middle row | Captured a title and its `ytd-rich-item-renderer` ancestor through the builder; chose a captured title selector, explicit document container, row-major order and codes `1`–`9`. Initial capture and full review completed, followed by a crash before a preview receipt was saved. A new normal-runtime profile imported the exact unchanged reviewed rule: preview reported 1–6 Live and 7–9 Missing, valid with activation prohibited. After explicit review/application, 7 did not navigate and 1 opened the intended first video. The earlier crash remains unresolved. |
| YouTube sidebar | Not checked live | Remains open. |

The opt-in examples are [Apple navigation](live/apple-uk-navigation.toml),
[Reddit home posts](live/reddit-home-posts.toml) and
[YouTube player](live/youtube-watch-player.toml). None is installed automatically.
Apple and Reddit use family `all`, rather than the default `actions` invocation.
The player pattern covers `/watch` regardless of video query; review that scope
before applying. The Reddit example is the actual native export. The player file
was transcribed from the exact native reviewed rule; its native export did not
complete. Both files pass the production parser, equality against their reviewed
rules and canonical round trip. The [channel-grid example](live/youtube-channel-grid.toml) was likewise transcribed
from its exact reviewed rule and checked with the production parser/equality/round trip.
It retains the earlier crash and the scope of its pre-mitigation live replay.

Capture and preview again before applying: one variant does not establish future
site stability. Mobile variants, other accounts, dynamic scrolling/reassignment,
physical keyboard delivery and Hint-context media delivery were not qualified.
No login, history setting, account state, voting or subscription was changed.
Only optional-cookie rejection was authorized and performed in disposable profiles.

Runs used the qualified Qt 6.11.2 build, owned Wayland backend, software rendering,
disabled userscripts and disposable private profiles. Fullscreen dimensions were
1280 × 720 logical pixels at compositor scale 1.5. The primary web document was
1280 × 694 at (0, 26), default zoom. Input used `wtype` virtual Wayland keys and
native AT-SPI control focus. This is not physical-hardware qualification.
No lock, login or CAPTCHA was bypassed.

Evidence under `target/site-policy-p05-revalidation/`:

- `p13-live-native-20261115-3`: Apple reviewed diff, preview, native export,
  safe-navigation screenshots and `live-checks.json`.
- `p13-live-native-20261115-4`: earlier Reddit Incomplete receipt and native
  export; YouTube player reviewed diff, preview and before/after screenshots.
  `youtube-raw-k.png` shows paused 0:38; `youtube-mapped-p.png` shows resumed;
  `youtube-player-exported.png` shows 3:33 but **does not prove export**.
- `p13-live-native-20261115-5`: current Reddit receipt, reviewed diff, missing-code
  and destination screenshots/path; grid capture/review; parser equality record;
  `crash-info.txt`, `crash-kernel.txt` and `crash-gdb.txt`.
- `p13-live-grid-replay-20261115-7`: normal-runtime unchanged-rule preview,
  review, explicit application, missing-code check and first-video navigation;
  normal confirmed quit from `about:blank`.
- `p13-preview-diagnostics-1`: additive bounded preview failure reasons/work
  counters, 140 actual-source Rust checks and nine native checks passed.

Apple/player used browser SHA-256
`61f45bedf837255b73da4276fd3d258d03c2b553fa15493a7624439e1ae84320`.
The current Reddit/grid run used
`1b7e3cd86bc100aedbe95d1fa34efa023a6bf514070c10fd3e6387d6aacf6ab9`.
Every run retains its source/build evidence separately.

An earlier grid capture attempt was invalidated by the test helper repeatedly
refocusing the same window, which warped the cursor onto a thumbnail and started
hover playback. Preserving focus/cursor allowed the later capture to complete.
This does not establish a product capture defect or qualify grid preview.

At 11:30:52 GMT the later grid preview crashed with SIGSEGV on the main thread in
Qt 6.11.2 `QV4::MarkStack::drain`, through `GCStateMachine::transition`. The kernel
record shows no OOM kill in that interval. Other threads were in event loops,
waits, rendering and owned workers. The stack does not establish the originating
invalid reference or whether its cause is application code or Qt. The temporary
extracted core was deleted after examination. The compositor restored prior focus.
No preview receipt was saved, and no grid policy was applied.
Further private-header/core inspection located a plain JavaScript Object whose
continuation slot was incorrectly present on the mark stack; it does not identify
the code that supplied that pointer. Qt's documented incremental-GC cross-validation
run passed 27 existing native preview/capture checks without reproducing the
live crash (`p13-qml-gc-diagnostic-1`). A subsequent exact-draft replay was blocked
by the desktop lock; no keys were sent and only its owned instance was closed.

The first live attempt encountered the desktop lock and sent no keys. The third
attempt also exposed a shutdown delay on YouTube consent; SIGTERM closed only
the verified test process. The fourth instance quit normally from `about:blank`.
Neither observation establishes a general shutdown fix. No machine/display
configuration was changed. Local fixtures cover broader navigation/list/grid/
sidebar/shadow/key cases; they do not replace the remaining live or P16 checks.

The later grid replay used the same pre-presentation-change browser
`1b7e3cd86bc100aedbe95d1fa34efa023a6bf514070c10fd3e6387d6aacf6ab9`.
It does not qualify the subsequently rebuilt presentation version. The original
crash remains retained even though this replay and normal quit passed.

The final production build now forces Qt's documented QML interpreter fallback
for Qt 6.11.2, avoiding the confirmed defective JIT SetLookup path. The crash's
invalid GC pointer is consistent with that upstream defect; its exact cause is
not uniquely proven. See [the workaround and tradeoff](../../docs/testing/qt6112-qml-jit-safety.md)
and [final qualification](../../docs/testing/site-policy-qualification.md).
The current build passed local native capture/preview, keyboard/page-test and
owned-Wayland regressions. No new live-site replay on that build is claimed.
