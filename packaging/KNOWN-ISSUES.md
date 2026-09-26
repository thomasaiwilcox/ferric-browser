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
- Grid mode is pre-alpha. Repeated native Wayland left-click and cleanup checks
  pass, but right-click, middle-click, canvas, cross-origin frame, scale/zoom,
  and prompt/fullscreen race coverage still need complete native qualification.
  Grid targets raw viewport coordinates and cannot track content moved by
  animation, reflow, canvas redraw, or inner-container scrolling.
- Screen-reader, keyboard-only dialog, and high-scaling accessibility
  qualification is incomplete.
- Linux on native Wayland is the only supported pre-alpha platform. X11,
  XWayland, macOS, Windows, and non-Arch packaging are unsupported.
