# M0-178: Wayland WebEngine surface recovery

Date: 2026-09-25
Status: qualified on the primary Arch/Hyprland/Omarchy target

## Failure boundary

On native Wayland with Qt/QtWebEngine 6.11.2 and accelerated OpenGL rendering,
switching away from a Ferric Browser workspace and back could leave only the
WebEngine content black. Browser chrome continued to render and the page
returned after pointer, scroll, or tab activity. Disabling Chromium GPU
compositing prevented the failure, while forcing ordinary Qt Quick repaint and
updating the WebEngine item tree did not.

QtWebEngine propagates visibility when the QML item changes visibility, but a
compositor workspace transition does not change that item property. Ferric now
uses a shared surface-recovery controller for primary, secondary, popup, and
developer-tools windows. After a previously presented native-Wayland window is
reactivated, the controller requests a frame and waits for `frameSwapped`. It
then sends a synchronous hide/show transition only to visible, active
WebEngine views and requests the resulting frame. Completing both transitions
in the same signal turn avoids presenting an intermediate detached surface.

The controller is disabled outside native Wayland and when Ferric's
`--software-rendering` mode is selected. It does not reload or navigate the
page, inject JavaScript, synthesize input, use a timer, call Qt private APIs, or
depend on Hyprland IPC.

## Automated verification

- Qt Quick tests cover initial presentation, activation/frame ordering,
  disabled operation, active lifecycle filtering, duplicate views, current-view
  replacement, cancellation, and repeated activation coalescing.
- Architecture checks require every WebEngine window family to use the shared
  controller and reject timers, deferred restoration, page scripts, reloads,
  and private APIs in that boundary.
- Required gates are `cargo xtask check`, `cargo xtask test engine`, and
  `cargo xtask test wayland`.

## Live qualification

The accelerated debug build was launched with a disposable profile against the
loopback editable fixture on Hyprland 0.56.2. Twenty consecutive workspace
round trips were captured at both 50 ms and 500 ms after returning, before any
page input. All 40 captures retained the rendered fixture and form contents.
The rejected deferred-restoration prototype produced one observable black
intermediate capture during tight cycling; this is why restoration remains
synchronous at the returned frame boundary.

The workaround may generate a brief page hidden/shown lifecycle transition
when an application focus return is not a workspace return. Qt does not expose
portable compositor workspace visibility, so this is the accepted tradeoff for
keeping the fix public-Qt-only and compositor independent.
