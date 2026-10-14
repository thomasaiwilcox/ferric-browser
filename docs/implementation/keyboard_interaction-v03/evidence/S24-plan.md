# S24 research plan

S24 asks whether a public accessibility-provider route can expose meaningful names, roles, bounds and actions for Qt Quick and Qt WebEngine content, preserve exact target invalidation, and coexist with a screen reader. The work stays in an opt-in test probe and synthetic local HTML fixture; it adds no production provider or page-side instrumentation.

The probe uses Qt 6.11.2 / Qt WebEngine 6.11.2, a native Wayland Qt Quick test window, a private session D-Bus and AT-SPI registry, and the public `atspi-2` client API. The private session uses a temporary runtime/config/data directory and an in-memory GSettings backend. The probe checks the private AT-SPI bus address before it enables its two status flags. The desktop session settings are read before and after.

The local fixture includes ordinary HTML controls, a labelled canvas, and a control in a closed shadow root. The QML test adds one named button whose safe action increments a local counter, then destroys that item after the client captures it. The AT-SPI client records names, roles, action interfaces, component extents, tree membership, and stale-action behavior. The HTML heading is checked from JavaScript so a successful WebEngine load is not mistaken for a successful accessibility-tree result.

The existing repository suites remain authoritative for T035 boundaries. S24 runs `cargo xtask check`, the engine/native gate `cargo xtask test engine`, and the focused `cargo xtask test accessibility` contract. Cross-origin and closed-root behavior is also checked by the existing WebEngine suite; native Wayland keyboard input remains separately qualified by the Wayland test runner, not by startup alone.
