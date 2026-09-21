# M0-148 — Typed tab-open navigation

Implemented the V1 tab-open [--background] [--] <input...> contract.

- The command registry exposes the required input and optional background
  argument, with URL completion and examples.
- Core resolves the complete URL/search input once and reduces an atomic
  OpenTabWithNavigation event. Foreground opens activate the new tab;
  background opens retain the current window and tab focus.
- Interactive commands, typed IPC, and CLI forwarding share the same bounded
  flag and input semantics. Typed IPC returns the allocated tab ID.
- The Qt adapter synchronizes the tab order before emitting the native
  navigation action, so the new WebEngine view is available to QML.

Evidence:

- cargo fmt --all -- --check
- cargo test -p browser-core --locked --offline: 59 passed
- cargo test -p rustbrowser --locked --offline: 42 passed
- cargo test -p browser-engine-qt --locked --offline: 113 passed
- cargo xtask check --locked
- Wayland startup smoke with --temp-basedir
