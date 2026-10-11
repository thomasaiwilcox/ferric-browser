# Webpage shortcuts

Focus the webpage or player, then choose how long to give it your keys.
Neither entry command selects an arbitrary input or changes page focus.

| Key in Normal mode | Indicator | Behavior |
| --- | --- | --- |
| `Ctrl+v` | `PASS THROUGH` | Forward ordinary keys until Ctrl+Escape. |
| `Ctrl+Shift+v` or `; v` | `PASS NEXT KEY` | Forward one key gesture, then return to Normal. |

For YouTube, focus the player, press Ctrl+v, then use its player shortcuts.
The browser's Normal-mode bindings are bypassed: `f` reaches the player instead
of starting hints. Ordinary Escape reaches the page to close a menu or exit
video fullscreen. Press **Ctrl+Escape** to return to Normal.

For an occasional shortcut, press Ctrl+Shift+v (or `;`, then `v`), release
the entry shortcut, and press `f`. Its press, held-key repeats, and final
release reach the page unchanged. Normal resumes after release. You can also
forward a modified key, Enter, or ordinary Escape; modifier keys alone do not
finish this mode. Ctrl+Escape cancels it without delivering Escape to the page.
Keys pressed while the selected key is held retain their repeats and release
even after Normal resumes.

The status bar shows the active variant and a `Ctrl+Esc: Normal` hint while
enabled by your existing status-bar setting. Hover over the mode for details.
The same behavior applies in primary and secondary browser windows.

Both variants reset when changing tabs, navigating, or closing the page view.
One-key forwarding also cancels when its window loses activation, so a lost
release cannot leave it armed. Persistent Pass-through survives deactivation.

The existing Ctrl+Shift+Escape emergency shortcut keeps its current behavior:
leave browser fullscreen when active, otherwise return to Normal. Enabled
desktop media-key handling also remains active. Operating-system shortcuts
remain under the desktop's control.

The entry commands are `mode-enter passthrough` and
`mode-enter passthrough-once`; `pass-through` and `pass-through-once` are
accepted aliases. Entry bindings can be changed through normal binding
configuration. Ctrl+Escape is reserved for recovery in either variant.
One-key forwarding is an explicit mode, not an automatic page-entry setting.

Text entry continues to use `i` and `I`, with ordinary Escape returning to
Normal. See [Text entry](insert-modes.md).
