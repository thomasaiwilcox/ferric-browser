# M0-158 — Primary live-tab detach path

The Qt adapter now creates primary tab views from a persistent pool rather than
letting a `Repeater` own their lifetime. `tab-detach` can therefore remove the
source model entry, reparent the existing `WebEngineView` into a new same-profile
window, adopt the tab metadata without navigation, and leave a blank fallback
tab in the source window.

The destination reuses the source `WebEngineProfile` and request interceptor;
the transfer view keeps its live engine object and is not reopened from its URL.
Primary-to-existing-secondary `tab-give` routing is documented separately in
M0-159; secondary-window sources remain an explicit follow-up boundary.

Evidence:

- `cargo fmt --all`
- `cargo check -p ferric-browser-engine-qt` — passed
- `cargo build -p ferric-browser --locked` — passed
- `cargo xtask check --locked` — passed (all workspace tests and doc-tests)
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir` —
  exited with the expected timeout status `124` and empty output.
- Manual native Wayland run with `--basedir /tmp/ferric-browser-m0-158-live`:
  - `ferric-browser command -- tab-detach` was accepted by the running instance.
  - The subsequent `query tabs --format json` reported the live tab in a new
    same-profile window (`window_id: windowid-2`) without navigation.
  - Repeated smoke run produced no new transfer-path errors; only the existing
    Qt WebEngine deprecation/dictionary and startup-property warnings appeared.
