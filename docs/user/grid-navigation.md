# Grid navigation

Grid mode lets you place a pointer action at a visible point in the current
page without moving the physical mouse. It is useful for canvases, image
regions, custom controls, and other targets that ordinary element hints cannot
identify precisely. Prefer normal hint mode for links and controls that already
have a useful semantic target.

## Quick start

1. From Normal mode, press `;g` or run `:grid`.
2. Press the number shown over the region that contains your target.
3. Repeat until the crosshair is over the intended point.
4. Press `Enter` to left-click the crosshair.

Each number selects one cell of the currently outlined rectangle:

```text
1 2 3
4 5 6
7 8 9
```

The grid is redrawn inside the selected cell after every refinement. The
crosshair always marks the exact point where a pointer action will be sent.
Pressing `Enter` acts on the current crosshair; it does not automatically select
cell 5. A successful click ends Grid mode and returns normal page input.

## Grid-mode keys

| Key | Result |
| --- | --- |
| `1`–`9` | Refine into the displayed cell without clicking. |
| `Enter` | Left-click the crosshair and leave Grid mode. |
| `Shift+Enter` | Right-click the crosshair and leave Grid mode. |
| `Ctrl+Enter` | Middle-click the crosshair and leave Grid mode. |
| `Space` | Hover at the crosshair, then reset to the full viewport while remaining in Grid mode. |
| `Backspace` | Return to the previous rectangle. |
| `0` | Reset immediately to the full viewport. |
| `?` | Toggle the concise in-browser help. |
| `Escape` | Cancel without sending another pointer action. |

`Shift` and `Ctrl` select the button action; they are not forwarded to the page
as pointer modifiers. A middle click or right click still follows the page and
browser's normal handling for that button.

## Hovering and moving pages

Use `Space` when a menu or control must first be hovered. After the hover is
delivered, inspect the updated page and refine from the full viewport again.
`Escape` cancels the current selection but cannot undo a hover effect already
sent to the page.

Grid mode addresses raw viewport coordinates, not a remembered page element.
Animation, reflow, canvas redraws, script-driven scrolling, and scrolling
inside page containers can move content after you aim. If the target moves,
cancel and start a new Grid session rather than committing a stale point.

Ferric also cancels Grid mode when the active tab, document, view, window,
geometry, browser prompt, renderer, or physical pointer invalidates the
captured page surface. This fail-closed behavior prevents a pending action from
being silently redirected to different content.

## Current qualification status

Grid mode is a pre-alpha feature. Repeated native Wayland left-click sessions
and cleanup have been exercised, but right-click, middle-click, canvas,
cross-origin frame, scale/zoom, and prompt/fullscreen race coverage still need
complete native-desktop qualification. See `KNOWN-ISSUES.md` in an installed
package or `docs/testing/spatial-grid-native-input.md` in the source tree for
the current evidence.

Grid mode requires visual spatial judgement and is not a substitute for
semantic accessibility. Normal hints and keyboard focus remain preferable when
the page exposes an accessible target.
