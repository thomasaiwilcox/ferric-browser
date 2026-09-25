# M0-179 mode-aware status-bar visibility

Ferric's default bottom-chrome policy is `in-mode`: normal browsing gives the
page the full window height, while insert, hint, caret, and pass-through modes
show the compact status bar. Command and search modes show their input surface
in the same bottom slot. `ui.statusbar = "always"` keeps the status bar visible
outside command/search input, and `"never"` suppresses the status surface while
leaving command and search input usable.

The behavior follows qutebrowser's documented `statusbar.show` policy names and
`in-mode` semantics. Qutebrowser itself documents `always` as its upstream
default; Ferric deliberately chooses `in-mode` as its product default. Ferric's
old `command` value remains accepted and is normalized to `in-mode` so existing
configuration does not fail after upgrade.

The policy is projected once from validated Rust configuration and evaluated by
the pure `ChromePresentation.statusBarVisible` helper. Primary, secondary, and
popup windows consume that shared decision. Secondary and popup web surfaces
also reclaim the bottom slot while the bar is hidden, avoiding a permanent
blank strip.

Automated coverage includes configuration defaults and validation, legacy-value
normalization, Settings-surface options, pure QML policy behavior for every
browser mode, and static wiring checks for all browser window types. Required
gates are `cargo xtask check` and `cargo xtask test engine`; native behavior is
qualified with the installed Wayland build.

On 2026-09-25, a disposable native-Wayland instance confirmed that normal mode
reclaims the bottom slot, insert mode displays the status bar, command mode
displays the command/completion surface, and a startup
`--set ui.statusbar=always` override restores the persistent normal-mode bar.

Reference: <https://qutebrowser.org/doc/help/settings.html#statusbar.show>
