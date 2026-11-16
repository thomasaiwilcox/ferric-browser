# YouTube and Reddit starter policies

These opt-in policies use Ferric's existing **`f` key**. From Normal mode,
press `f`, then the displayed code. Search, navigation, numbered items and
ordinary controls share that same hint session. No binding change is needed.

| Code | YouTube | Reddit |
| --- | --- | --- |
| `h` | Home | Home |
| `s` | Focus Search | Focus Search |
| `u` | Subscriptions on browsing/search pages, when visible | — |
| `1`–`9` | Visible videos, search results or watch-page recommendations | Visible feed post titles |

Other targets keep automatic hint labels. Numbers refer to the visible
snapshot when hints begin. Missing positions stay reserved. Start a new hint
session after scrolling to take a new snapshot; a reserved code cannot silently
move to a different item within the same session.

`youtube-browse.toml` covers browsing grids and search results.
`youtube-watch.toml` replaces those collections on `/watch` with recommendations
and admits the player's `j`, `k`, `l`, `m`, `f` and `c` keys while hints are open.
Its `p` alias sends `k` for play/pause in Normal, Hint or passthrough, subject to
native receiver admission. Direct Normal-mode delivery also needs the existing
watch-scoped settings `input.shortcut_policy = "mixed"` and
`input.page_keys = ["p"]`; the Interaction interchange file cannot carry ordinary
settings. The installed setup configures those through the settings service.
Normal-mode `f` still opens hints. With the page focused, press `p` directly;
typing fields retain their normal input. On watch pages this deliberately
replaces the `pp`/`pP` browser prefixes with the immediate `p` shortcut.
Inside ordinary hints, one admitted player shortcut returns to Normal after
release. Thus `f`, then `p` changes playback and dismisses hints; `f`, then `f`
changes fullscreen and dismisses hints. Use `;V` for consecutive player keys.
Blocked shortcuts leave Hint mode intact, and hint text queries remain text.
For other player keys, [YouTube documents their meanings](https://support.google.com/youtube/answer/7631406?hl=en).
`reddit-reading.toml` targets current desktop feed titles and the search field
inside two open shadow roots. It adds no voting or posting shortcuts.

## Import and try

The corrected combined bundle is
`~/.local/share/ferric-browser/presets/youtube-reddit-starter-20261115/all.toml`.
It was exported by the production service. The corrected profiles have been
saved for the next browser start; restart Ferric after installing the update.
The earlier narrow-family presets have been superseded.

Open `:site-policy`, import a file into **temporary**, review the exact rule IDs
and diff, preview on the intended tab, then apply the reviewed revision.
Importing alone does not activate it. Both YouTube files compose in the same
source. Higher source layers take precedence over their numeric priorities.
Enhanced labels require inherited `full-match` or `never` auto-follow; preview
reports an incompatible setting before application.

## Verification

Selectors were researched on signed-out desktop `www.youtube.com` and
`www.reddit.com` on 15 November 2026. The default-`f` correction build was checked through
its unmodified built-in `f` binding using an owned Wayland seat at 640 × 480:

- YouTube search: `f`, then `1` opened the exact visible video.
- YouTube watch: `f`, then `1` opened the exact visible recommendation. Its
  recommendation container follows both sidebar and stacked responsive layouts.
- Reddit: `f`, then `s` focused the exact search textarea through two shadow
  roots; a fresh `f`, then `1` opened the expected visible post.
- The preceding build also checked a channel-grid selection and confirmed a
  Missing `9` reservation did not navigate. Normal `f` was never rebound.

The corrected action-family previews completed without unavailable rules on
search, watch and Reddit. Predicate work was 83,947, 53,407 and 22,110 against the
unchanged 100,000 grant. The first watch-player `p` → `k` check was
unqualified; that historical failure remains in those artifacts. The subsequent
player-key correction is qualified separately below.

The earlier links/input-only trials did not qualify the intended `f` workflow.
Their failures remain in `target/site-policy-preset-research-20261115/`.
Correction evidence is under `target/site-policy-default-f-correction-1/` and
`target/site-policy-default-f-correction-2/`. The pre-existing 5,000-target
collector timeout remains within the user's deferred extreme-load work.

A signed-out YouTube homepage with history off may contain no videos. Hidden
navigation and offscreen items legitimately have no live fixed code. Signed-in,
mobile, old Reddit, other locales and physical keyboard input need their own
qualification. Site markup can change; recapture and preview if a target
becomes absent or ambiguous.

The play/pause correction in `target/site-policy-player-key-fix-1/` passed
18 targeted native checks and seven interchange checks. Its causal regression
reproduced Escape suppressing fresh noneditable receiver facts with an unchanged
DOM sequence. The fix retains automatic Insert suppression and unsafe-focus
blocking.

On the exact installed binary, a fresh signed-out YouTube watch page received
trusted native `k` press/release pairs from `p`: Normal-mode presses paused and
resumed the video, and Hint-mode `p` also changed playback while `f` retained its
built-in command. A focused Search field received literal `p` with playback
unchanged. The Normal mapping was absent on channel pages. A later refocus
attempt without observed key events remains inconclusive and was not used as
positive delivery evidence; the final fresh-session receipt contains the
trusted events and playback transitions. Remapping remains blocked in editable
or unknown receiver contexts.
