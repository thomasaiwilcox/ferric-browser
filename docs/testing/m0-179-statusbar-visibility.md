# M0-179 mode-aware status-bar visibility

Ferric's default bottom-chrome policy is `in-mode`: normal browsing hides the
status bar, while insert, hint, caret, and pass-through modes show it. Command
and search modes show their input surface in the same bottom slot. These bars
overlay the page without changing its viewport. `ui.statusbar = "always"` keeps
the status bar visible outside command/search input, and `"never"` suppresses
the status surface while leaving command and search input usable.

The behavior follows qutebrowser's documented `statusbar.show` policy names and
`in-mode` semantics. Qutebrowser itself documents `always` as its upstream
default; Ferric deliberately chooses `in-mode` as its product default. Ferric's
old `command` value remains accepted and is normalized to `in-mode` so existing
configuration does not fail after upgrade.

The policy is projected once from validated Rust configuration and evaluated by
the pure `ChromePresentation.statusBarVisible` helper. Primary, secondary, and
popup windows consume that shared decision. All three window types reserve
space for persistent tab strips and profile-health notices independently of
the transient bars. The bars sit above that persistent chrome and cover the
bottom of the page while visible; no blank slot remains when they are hidden.

Automated coverage includes configuration defaults and validation, legacy-value
normalization, Settings-surface options, pure QML policy behavior for every
browser mode, and static wiring checks for all browser window types. Required
gates are `cargo xtask check` and `cargo xtask test engine`; native behavior is
qualified with the installed Wayland build.

On 2026-09-25, a disposable native-Wayland instance confirmed that normal mode
reclaims the bottom slot, insert mode displays the status bar, command mode
displays the command/completion surface, and a startup
`--set ui.statusbar=always` override restores the persistent normal-mode bar.

## Overlay follow-up — 2026-10-21

UI-001 now explicitly requires a stable page viewport when status, command,
search, or command-feedback surfaces appear or disappear. Previously their
height was included in the WebEngine layout's bottom margin, causing page
reflow and visible movement during mode changes. It also shifted bottom tab
strips and profile notices. Only persistent chrome now contributes to the
page's bottom inset. The status and input surfaces consume pointer and wheel
input over their backgrounds so covered page content cannot receive it.
Copy feedback is offset above the bars, keeping its actions unobscured.

`ApplicationTests::bottomChromeOverlay` exercises the production primary,
secondary, and popup windows with top, bottom, and left tab strips, including
a persistent profile notice with bottom tabs. It checks WebEngine geometry,
renderer viewport dimensions, scroll position, a fixed footer, resize-event
counts, notice position, status visibility policies, and clicks over the
status-bar background through insert, command, search, and feedback transitions.
It is included in `application_navigation_usability` and
`application_bottom_chrome_wayland`. The latter owns a private compositor and
uses compositor-delivered pointer clicks; its keyboard setup uses Qt events.

Verification on 2026-10-21 (Qt 6.11.2, software rendering):

- The regression against the previous build observed a 28-logical-pixel page
  shrink and eight renderer resize events during chrome transitions.
- `cargo xtask check --locked --offline` passed, including all 971 QML checks
  and the Rust workspace tests.
- `cargo xtask test adapter`, workspace Clippy with `-D warnings`, formatting,
  and `git diff --check` passed. The copy-notice QML lint reported no critical
  diagnostics, and its focused prompt suite passed all 108 checks.
- `application_page_search` and `application_navigation_usability` passed
  against the final release build (140.63 seconds combined).
- `application_bottom_chrome_wayland` passed all nine window/layout cases with
  real seat pointer clicks (39.17 seconds). The final focused offscreen overlay
  run also passed all nine cases (37.78 seconds).

The pointer fixture chooses page coordinates away from the responsive Copy URL
button and page scrollbars, and explicitly checks that the button is excluded.
This keeps narrow-window tests focused on the bar background rather than
opening copy feedback over the next test click.

Reference: <https://qutebrowser.org/doc/help/settings.html#statusbar.show>
