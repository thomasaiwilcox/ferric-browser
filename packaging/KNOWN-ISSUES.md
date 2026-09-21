# Known issues

- Native Wayland qualification requires `weston` and `dbus-run-session`; the
  disposable harness reports unavailable when either is absent.
- Long-duration fuzzing, release-hardware performance qualification, and
  screen-reader/portal qualification remain release acceptance work.
- QtWebEngine Chromium security-patch metadata is not exposed by the installed
  package probe, so diagnostics do not claim a patch level.
- Source archives, checksums, signatures, and installed-file manifests are
  generated and reviewed by release automation.
- Cross-origin frame hint collection and activation remain incomplete.
- Screen-reader, keyboard-only dialog, and high-scaling accessibility
  qualification is incomplete.
- Linux on native Wayland is the only supported pre-alpha platform. X11,
  XWayland, macOS, Windows, and non-Arch packaging are unsupported.
